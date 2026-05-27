//! Phase 1 并发下载的可见性外壳——固定高度 region, 用 crossterm 原地刷新。
//!
//! 非 TTY / JSON 模式: 不画 region, 每个事件直接通过 status_* 打一行——
//! CI 和管道日志友好。

use std::collections::HashMap;
use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{interval, Duration};

use crate::downloader::events::ProgressEvent;

/// 每行最多展示的字段宽度——和现有 status 行 12 字符 verb 对齐。
const VERB_WIDTH: usize = 12;

/// UI 入口。jobs = 期望同时跑的最大 task 数, 决定 region 高度。
pub async fn run(
    mut rx: UnboundedReceiver<ProgressEvent>,
    jobs: usize,
    cancel: Arc<AtomicBool>,
) {
    let tty = std::io::stderr().is_terminal() && !crate::display::is_json_mode();
    if !tty {
        // Headless: 每事件打一行
        while let Some(ev) = rx.recv().await {
            if cancel.load(Ordering::SeqCst) {
                break;
            }
            print_event_plain(&ev);
        }
        return;
    }

    let mut state: HashMap<String, RowState> = HashMap::new();
    let region_height = jobs + 1; // 1 header + jobs 行
    let mut painter = RegionPainter::new(region_height);
    let mut tick = interval(Duration::from_millis(100));

    loop {
        tokio::select! {
            biased;
            _ = tick.tick() => {
                if cancel.load(Ordering::SeqCst) { break; }
                painter.paint(&state);
            }
            ev = rx.recv() => {
                match ev {
                    Some(ev) => apply_event(&mut state, &mut painter, ev),
                    None => break, // sender 全 drop, 收尾
                }
            }
        }
    }

    painter.clear();
}

fn print_event_plain(ev: &ProgressEvent) {
    use crate::display::status_dim;
    match ev {
        ProgressEvent::Resolving { name } => status_dim("Binstall", &format!("{name} resolving")),
        ProgressEvent::UrlCandidate { .. } => {} // 太冗余, 不打
        ProgressEvent::Downloading { name, got, total } => {
            let pct = total.map(|t| format!("{}%", got * 100 / t)).unwrap_or_default();
            status_dim("Binstall", &format!("{name} downloading {pct}"));
        }
        ProgressEvent::Verifying { name } => status_dim("Binstall", &format!("{name} verifying")),
        ProgressEvent::Extracting { name } => status_dim("Binstall", &format!("{name} extracting")),
        ProgressEvent::Installing { name } => status_dim("Binstall", &format!("{name} installing")),
        ProgressEvent::Done { name, version } => {
            crate::display::status("Updated", &format!("{name} -> {version}"));
        }
        ProgressEvent::Failed { name, reason } => {
            crate::display::status_warn("Fallback", &format!("{name} ({reason})"));
        }
    }
}

#[derive(Clone)]
pub struct RowState {
    pub name: String,
    pub phase: Phase,
    pub got: u64,
    pub total: Option<u64>,
    pub started: Instant,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Queued,
    Resolving,
    Downloading,
    Verifying,
    Extracting,
    Installing,
    Done,
    Failed,
}

fn make_row(name: &str, phase: Phase) -> RowState {
    RowState {
        name: name.to_string(),
        phase,
        got: 0,
        total: None,
        started: Instant::now(),
    }
}

fn apply_event(
    state: &mut HashMap<String, RowState>,
    painter: &mut RegionPainter,
    ev: ProgressEvent,
) {
    match &ev {
        ProgressEvent::Resolving { name } => {
            let r = state
                .entry(name.to_string())
                .or_insert_with(|| make_row(name, Phase::Resolving));
            r.phase = Phase::Resolving;
        }
        ProgressEvent::UrlCandidate { .. } => {}
        ProgressEvent::Downloading { name, got, total } => {
            let r = state
                .entry(name.to_string())
                .or_insert_with(|| make_row(name, Phase::Downloading));
            r.phase = Phase::Downloading;
            r.got = *got;
            r.total = *total;
        }
        ProgressEvent::Verifying { name } => {
            let r = state
                .entry(name.to_string())
                .or_insert_with(|| make_row(name, Phase::Verifying));
            r.phase = Phase::Verifying;
        }
        ProgressEvent::Extracting { name } => {
            let r = state
                .entry(name.to_string())
                .or_insert_with(|| make_row(name, Phase::Extracting));
            r.phase = Phase::Extracting;
        }
        ProgressEvent::Installing { name } => {
            let r = state
                .entry(name.to_string())
                .or_insert_with(|| make_row(name, Phase::Installing));
            r.phase = Phase::Installing;
        }
        ProgressEvent::Done { name, version } => {
            state
                .entry(name.to_string())
                .or_insert_with(|| make_row(name, Phase::Done))
                .phase = Phase::Done;
            // 把"Updated <name> -> <version>"持久化到 region 之上
            painter.println_above(&format!(
                "{verb:>width$} {name} -> {version}",
                verb = "Updated",
                width = VERB_WIDTH,
            ));
            state.remove(name);
        }
        ProgressEvent::Failed { name, reason } => {
            state
                .entry(name.to_string())
                .or_insert_with(|| make_row(name, Phase::Failed))
                .phase = Phase::Failed;
            painter.println_above(&format!(
                "{verb:>width$} {name} ({reason})",
                verb = "Fallback",
                width = VERB_WIDTH,
            ));
            state.remove(name);
        }
    }
}

/// 控制 stderr 上一段 N 行 region 的画家。
///
/// 第一次 `paint` 时把光标位置记下来 (当时光标在哪, region 就从哪开始),
/// 后续每次 paint 先把光标移回 region 顶, clear region, 再写新内容。
/// 终止时 `clear` 把整个 region 抹掉, 让后续 status 行从 region 起始位置
/// 继续 (region 之上的持久化行通过 println_above 已经 scroll 进了历史)。
struct RegionPainter {
    height: usize,
    initialized: bool,
}

impl RegionPainter {
    fn new(height: usize) -> Self {
        Self {
            height,
            initialized: false,
        }
    }

    fn paint(&mut self, state: &HashMap<String, RowState>) {
        use crossterm::{cursor, terminal, QueueableCommand};
        let mut out = std::io::stderr();
        if !self.initialized {
            // 第一次 paint: 在当前光标位置开辟 N 行空间, 写入, 把光标停在 region 顶
            for _ in 0..self.height {
                let _ = writeln!(out);
            }
            let _ = out.queue(cursor::MoveUp(self.height as u16));
            self.initialized = true;
        }
        // 先暂存光标 (它在 region 顶), 写一遍, 再回到原位
        let _ = out.queue(cursor::SavePosition);
        for line in render_lines(state, self.height) {
            let _ = out.queue(terminal::Clear(terminal::ClearType::CurrentLine));
            let _ = writeln!(out, "{line}");
        }
        let _ = out.queue(cursor::RestorePosition);
        let _ = out.flush();
    }

    fn println_above(&mut self, line: &str) {
        use crossterm::{cursor, terminal, QueueableCommand};
        let mut out = std::io::stderr();
        if !self.initialized {
            // region 还没建立, 直接打
            let _ = writeln!(out, "{line}");
            return;
        }
        // 在 region 顶端正上方"插一行": 把 region 整体下移一行的方式比较复杂,
        // 简化策略: clear region, 打一行 (它会滚进历史), 重新建立 region
        let _ = out.queue(cursor::SavePosition);
        for _ in 0..self.height {
            let _ = out.queue(terminal::Clear(terminal::ClearType::CurrentLine));
            let _ = writeln!(out);
        }
        let _ = out.queue(cursor::RestorePosition);
        let _ = writeln!(out, "{line}");
        // 重新预留 region 空间
        for _ in 0..self.height {
            let _ = writeln!(out);
        }
        let _ = out.queue(cursor::MoveUp(self.height as u16));
        let _ = out.flush();
    }

    fn clear(&mut self) {
        if !self.initialized {
            return;
        }
        use crossterm::{cursor, terminal, QueueableCommand};
        let mut out = std::io::stderr();
        let _ = out.queue(cursor::SavePosition);
        for _ in 0..self.height {
            let _ = out.queue(terminal::Clear(terminal::ClearType::CurrentLine));
            let _ = writeln!(out);
        }
        let _ = out.queue(cursor::RestorePosition);
        let _ = out.flush();
    }
}

/// 把 state 渲染成正好 `height` 行 (不足补空, 超长截断)。
fn render_lines(state: &HashMap<String, RowState>, height: usize) -> Vec<String> {
    let mut active: Vec<&RowState> = state.values().collect();
    active.sort_by_key(|a| a.started);
    let mut lines = Vec::with_capacity(height);

    // header
    lines.push(format!(
        "{verb:>width$} {n} in flight",
        verb = "Binstalling",
        width = VERB_WIDTH,
        n = active.len(),
    ));

    for r in active.iter().take(height - 1) {
        lines.push(format_row(r));
    }
    while lines.len() < height {
        lines.push(String::new());
    }
    lines
}

pub fn format_row(r: &RowState) -> String {
    let elapsed = r.started.elapsed().as_secs();
    let phase = match r.phase {
        Phase::Queued => "queued",
        Phase::Resolving => "resolving",
        Phase::Downloading => "downloading",
        Phase::Verifying => "verifying",
        Phase::Extracting => "extracting",
        Phase::Installing => "installing",
        Phase::Done => "done",
        Phase::Failed => "failed",
    };
    let progress = if matches!(r.phase, Phase::Downloading) {
        match r.total {
            Some(t) if t > 0 => format!(" {}/{}KB {}%", r.got / 1024, t / 1024, r.got * 100 / t),
            _ => format!(" {}KB", r.got / 1024),
        }
    } else {
        String::new()
    };
    format!(
        "{name:<24} {phase}{progress} {elapsed}s",
        name = r.name,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_row_downloading_includes_progress() {
        let r = RowState {
            name: "ripgrep".into(),
            phase: Phase::Downloading,
            got: 1024 * 100,
            total: Some(1024 * 200),
            started: Instant::now(),
        };
        let s = format_row(&r);
        assert!(s.contains("ripgrep"), "got: {s}");
        assert!(s.contains("downloading"), "got: {s}");
        assert!(s.contains("100/200KB"), "got: {s}");
        assert!(s.contains("50%"), "got: {s}");
    }

    #[test]
    fn format_row_queued_omits_progress() {
        let r = RowState {
            name: "x".into(),
            phase: Phase::Queued,
            got: 0,
            total: None,
            started: Instant::now(),
        };
        let s = format_row(&r);
        assert!(!s.contains("KB"));
        assert!(!s.contains("%"));
    }

    #[test]
    fn render_lines_pads_to_height() {
        let st = HashMap::new();
        let lines = render_lines(&st, 5);
        assert_eq!(lines.len(), 5);
        assert!(lines[0].contains("Binstalling"));
        // 后 4 行是空
        for (i, line) in lines.iter().enumerate().skip(1) {
            assert!(line.is_empty(), "line {i}: {:?}", line);
        }
    }
}
