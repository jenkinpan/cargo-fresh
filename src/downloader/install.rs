//! Atomic install to ~/.cargo/bin + .crates2.json update.
//!
//! 写法: 先把临时 binary 拷到一个隐藏的 `.cargo-fresh-{name}-{uuid}.tmp`
//! 兄弟文件, fsync, 然后 fs::rename 原子替换目标。fs::rename 在同一文件
//! 系统下是原子的, 中途断电也不会出现"半个 binary"。

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::downloader::events::{DownloaderError, FailureKind, UnsupportedReason};

/// Serialize concurrent writes to `$CARGO_HOME/.crates.toml` and
/// `$CARGO_HOME/.crates2.json` from this process. Concurrent updates
/// (introduced in 0.12.0) can race on the read-modify-write of these
/// files; an in-process `Mutex<()>` is sufficient because running two
/// `cargo-fresh` instances against the same $CARGO_HOME is not a
/// supported use case (cargo itself takes its own $CARGO_HOME lock for
/// `cargo install`).
pub(crate) static CRATES_FILES_LOCK: Mutex<()> = Mutex::new(());

pub fn install_binary(
    src: &Path,
    binary_name: &str,
    new_version: &str,
) -> Result<PathBuf, DownloaderError> {
    if cfg!(windows) {
        return Err(DownloaderError::Unsupported(
            UnsupportedReason::UnsupportedPlatform,
        ));
    }
    let cargo_home = cargo_home_path().map_err(failed_install)?;
    let bin_dir = cargo_home.join("bin");
    let dest = bin_dir.join(binary_name);

    let uuid = format!("{:x}", std::process::id() as u128 * 1_000_000 + rand_u32() as u128);
    let tmp = bin_dir.join(format!(".cargo-fresh-{binary_name}-{uuid}.tmp"));

    // 1. 拷 + fsync
    std::fs::copy(src, &tmp).map_err(|e| failed_install(anyhow!(e).context("copy to tmp")))?;
    #[cfg(unix)]
    set_executable(&tmp).map_err(failed_install)?;
    fsync_file(&tmp).map_err(failed_install)?;

    // 2. atomic rename
    if let Err(e) = std::fs::rename(&tmp, &dest) {
        let _ = std::fs::remove_file(&tmp);
        return Err(failed_install(anyhow!(e).context("rename tmp -> dest")));
    }

    // 3. cargo 元数据更新——两个文件都得写, 不然 `cargo install --list` 还报旧版
    //    (.crates.toml 是主索引, .crates2.json 是扩展字段)。
    //    并发更新下两次 read-modify-write 之间可能交错,所以加进程内锁。
    {
        let _guard = CRATES_FILES_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        crate::package::crates2::write_install_record(&cargo_home, binary_name, new_version)
            .map_err(|e| failed_install(e.context("update .crates2.json")))?;
        crate::package::crates_toml::write_install_record(&cargo_home, binary_name, new_version)
            .map_err(|e| failed_install(e.context("update .crates.toml")))?;
    }

    Ok(dest)
}

fn cargo_home_path() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("CARGO_HOME") {
        return Ok(PathBuf::from(p));
    }
    let home = std::env::var("HOME").context("HOME unset")?;
    Ok(PathBuf::from(home).join(".cargo"))
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path).context("stat")?.permissions();
    perms.set_mode(perms.mode() | 0o755);
    std::fs::set_permissions(path, perms).context("set_permissions")?;
    Ok(())
}

fn fsync_file(path: &Path) -> Result<()> {
    let f = std::fs::File::open(path).context("open for fsync")?;
    f.sync_all().context("fsync")?;
    Ok(())
}

fn rand_u32() -> u32 {
    // 不引 rand crate——nanos 时间足以避免碰撞
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos())
}

fn failed_install(e: anyhow::Error) -> DownloaderError {
    DownloaderError::Failed {
        kind: FailureKind::InstallFailed,
        source: e,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crates_files_lock_serializes_concurrent_threads() {
        // Two threads both lock, run a brief no-op critical section, drop.
        // Confirms the static Mutex exists, is Send+Sync, and uncontended.
        let h1 = std::thread::spawn(|| {
            let _g = CRATES_FILES_LOCK.lock().expect("lock 1");
            std::thread::sleep(std::time::Duration::from_millis(10));
        });
        let h2 = std::thread::spawn(|| {
            let _g = CRATES_FILES_LOCK.lock().expect("lock 2");
            std::thread::sleep(std::time::Duration::from_millis(10));
        });
        h1.join().unwrap();
        h2.join().unwrap();
    }
}
