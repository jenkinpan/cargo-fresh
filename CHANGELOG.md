# Changelog

所有重要的项目变更都会记录在这个文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
并且此项目遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased]

### Added

- **`--debug` 标志**：开启后向 stderr 输出 downloader 决策链的内部 trace —— 每包路径分支选择（downloader vs cargo install 的原因）、`repo_url` 解析结果、`.crates2.json` 里查到的 `bins[]`、GitHub Releases API 试探每个 tag 的结果（`200 matched=...` / `200 unmatched-of-N` / `404` / `error=...`）、token 来源（`env:GITHUB_TOKEN` / `env:GH_TOKEN` / `gh` / `none`）、API 命中或回退到 360 URL 枚举时的候选数。专门给"为什么这个包没用预编译"类型的 issue 报告使用，用户贴一份 `cargo fresh --debug 2>&1 | grep debug` 输出，足够还原下载器决策。与 `--format=json` 正交（JSON 在 stdout，debug 在 stderr，CI 里 `2>debug.log` 互不污染）。**不属于 1.0 稳定契约**——输出格式与字段可在任意版本调整，请不要 grep/解析具体串。

## [0.12.3] - 2026-05-29

### Added

- **`cargo fresh completion <shell> --install` 全面重写为交互式多 shell**：`--install` 现在弹出 MultiSelect 让用户挑选要装的目标（顶层 `cargo-fresh<TAB>` 补全 / cargo 子命令 `cargo fresh<TAB>` 补全；两项默认全选）。安装路径覆盖全部六个 shell —— bash（`~/.local/share/bash-completion/completions/`）、zsh（`~/.zfunc/`）、fish（`~/.config/fish/completions/`）、powershell（`~/.config/powershell/`）、elvish（`~/.config/elvish/lib/`）、nushell（`~/.config/nushell/completions/`），均尊重 `XDG_CONFIG_HOME` / `XDG_DATA_HOME`。对默认目录不在自动加载路径上的 shell（zsh / powershell / elvish / nushell），结束时打一行 `Hint` 给出确切要加入 `~/.zshrc` / `$PROFILE` / `rc.elv` / `config.nu` 的那行配置。
- **`completion --install --yes`**：新增 `--yes` 跳过 MultiSelect 直接装两个目标，便于脚本 / CI。非 TTY 自动等价于 `--yes`。
- 单包成功 / 跳过仍按原行已有的 `Installed` / `Skipped` 状态行打印；结束时新增一行 `Finished N written, M skipped` 汇总。

### Changed

- `completion <shell> --install --cargo-fresh` 中的 `--cargo-fresh` 与 `--install` 合用时被忽略（picker 已经覆盖两个目标）。不带 `--install` 的纯 stdout 重定向场景下，`--cargo-fresh` 行为不变。
- README.md / README.zh.md 全面重写：新增目录、用紧凑的 Highlights 段替换 emoji feature 列表、把原本三段重叠的 Shell completion 内容合并为一段（含新的 `--install` 交互流程与各 shell 安装路径对照表）、修正与当前动词格式不一致的示例输出、精简 License 段。中英文 1:1 同步。

### Removed

- 删除根目录下过时的 `completion/` 文件夹（孤儿 `.nu` 文件，遗留自项目早期 `pkg-checker` 名）和 `COMPLETION.md`（文档里仍在引用旧名 `pkg-checker`，且已与现行 CLI 失同步）。

无 BREAKING / 无 BEHAVIOR（旧的 `completion fish --install` 调用仍可用，非 TTY 现在等价于装两个目标 —— 严格超集，没有撤回任何能力）。

## [0.12.2] - 2026-05-28

MSRV 抬到 1.88。0.12.1 升级了 `zip` 到 8.x,而整个 zip 8 系列要求 rustc ≥ 1.88,导致 CI 的 MSRV (1.86) job 红了。无功能改动,无 BREAKING。

### Changed

- **MSRV 1.86 → 1.88**: `zip 8.x` 整系列要求 1.88。同步更新 `Cargo.toml::rust-version`、`.github/workflows/ci.yml` MSRV job、`CONTRIBUTING.md`、`CLAUDE.md`

## [0.12.1] - 2026-05-28

依赖跨大版本升级。无功能改动,无 BREAKING(对外 CLI / JSON schema 不变)。

### Changed

- **deps**: `anstream` 0.6 → 1.0;`clap_mangen` 0.2 → 0.3;`zip` 2 → 8(均为 drop-in)
- **deps**: `reqwest` 0.12 → 0.13。`features` 里的 `rustls-tls` 改为 `rustls`(0.13 重命名)。**行为变化**:TLS 信任根从捆绑的 `webpki-roots` 切换到平台验证器(macOS Keychain / Windows cert store / Linux 系统 CA),crypto backend 从 `ring` 变成 `aws-lc-rs`。企业 MITM 代理的自定义 CA 现在会被自动信任
- **deps**: `toml` 0.8 → 1.1。`features` 增加 `serde`(1.x 把 `Value` / `from_str` 拆到 `serde` feature 后面了)
- **deps**: `sha2` 0.10 → 0.11。`Digest::finalize()` 返回类型从 `GenericArray` 变成 `Array<u8, _>`,不再实现 `LowerHex`。`src/downloader/fetch.rs::compute_sha256` 改成手动逐字节 hex 编码
- **deps (transitive)**: `cargo update` 同时刷新了 17 个间接依赖(`hyper` 1.10、`serde_json` 1.0.150、`wasm-bindgen` 0.2.122 等)

## [0.12.0] - 2026-05-28

并发更新调度器——`--jobs N` 默认 4,`-j 0` 表示无限制。N 个包同时跑下载/解压/安装,`cargo install` fallback 自然在 cargo 的 `$CARGO_HOME` 锁上排队,无需额外构建池。MultiProgress 行按输入顺序预注册,完成顺序无关地保留屏幕顺序;summary 也按输入顺序重排。

### Added

- **`--jobs N` / `-j N`** (`src/cli/mod.rs`): 并发上限,默认 4,`0`=无限制。`-j 1` 退回 0.11 串行行为
- **`src/main.rs::run_one_update`**: per-package 更新被抽成自有所有权的 async 函数,适配 `JoinSet::spawn`
- **`tests/concurrent_smoke.rs`**: shape-level 测试覆盖串行/并行/乱序完成三种情况,锁住"`sort_by_key` 把结果还原成输入顺序"这一不变量
- **`src/downloader/github_api.rs`** + **`src/downloader/token.rs`**: GitHub Releases API client + token discovery。`--check-prebuilt` 和真正的下载都改成"先调 `GET /repos/{owner}/{repo}/releases/tags/{tag}` 拿 asset 清单,本地按文件名匹配命中后直接 stream GET"。匿名 60/hr,带 token (`$GITHUB_TOKEN` / `$GH_TOKEN` / `gh auth token`) 5000/hr

### Changed

- **BEHAVIOR**: 默认行为从串行更新切换为最多 4 包并发。`--jobs 1` 恢复旧行为
- **`.crates.toml` / `.crates2.json` 写入序列化** (`src/downloader/install.rs::CRATES_FILES_LOCK`): 进程内 `Mutex<()>` 兜住两个文件的 read-modify-write 竞态;两个并发包写不同 binary 行不会再丢失记录
- **Ctrl-C 双击语义**: 首次 Ctrl-C 软取消(显示 `Aborting Ctrl-C again to force exit`,在飞任务自然收尾);二次 Ctrl-C 立即 `exit(130)`。TempDir Drop + atomic rename 保证不留半安装态
- **CLI**: `--check-binstall` → `--check-prebuilt`。语义一致(标记每个候选包是预编译还是要从源码构建)
- **BEHAVIOR**: 预编译探测改用 downloader 自己的 HEAD probe,不再 spawn `cargo binstall --dry-run`。~10s/包 → 1-2s/包。也不再需要 cargo-binstall 装着
- **BREAKING (JSON)**: `schema_version` 1 → 2;`updates_available[].binstall` → `updates_available[].prebuilt`;enum 值 `source_build` → `source`。这是 1.0 前最后一次破约
- **BEHAVIOR**: probe + fetch 的 happy path 不再对 github.com 发 16-360 个 HEAD,改成 1-6 个 API 请求。API 失败 (401/403/429/网络) 时仍自动 fallback 到旧 HEAD 路径。给手动跑 100+ 包的用户彻底消灭 "突然 throttle" 那一类 flake

### Removed

- **`--install-binstall` flag**: 已弃用一个版本周期 (0.11),按计划在 0.12.0 移除。如果脚本里还带这个 flag 会被 clap 拒绝。换用 `cargo install cargo-binstall` 在 shell 里显式装
- `src/package/binstall_probe.rs` 整个模块(原来调 `cargo binstall --dry-run`)
- `package::is_binstall_available` / `install_binstall` / `ensure_binstall_available`
- 7 个 locale key: `binstall_hint` / `installing_binstall` / `attempting_to_install_binstall` / `binstall_not_found` / `binstall_installed_successfully` / `binstall_install_failed` / `checking_binstall`

### Notes for downstream

- JSON `schema_version` 1 → 2 (BREAKING):脚本消费者需要把字段名 `binstall` 改成 `prebuilt`,枚举值 `source_build` 改成 `source`。`results[]` 顺序仍跟选择顺序一致(由 `sort_by_key` 保证),工具链可以继续按位置读
- 退出码契约 0/1/2/130 不变
- `MultiProgress` 在非 TTY 自动降级(0.10.1 起),并发也不例外
- 想跑 `--check-prebuilt` 频繁(CI / 几十个包) 强烈建议 export `GITHUB_TOKEN`,或装 `gh` CLI 并 `gh auth login`。匿名 60/hr 对正常使用还是够,但对批量探测不够

## [0.11.0] - 2026-05-28

两段式 release：底层是用进程内下载器替换 `cargo binstall` 子进程；后半轮是验证后的覆盖率与 UX 收尾——把下载器从"能跑"推到"能跑得过 binstall"，UI 翻成 rustup 风格。没动 JSON schema、没改 CLI flag 表面。

### 验证轮收尾

### Added

- **`src/package/crates_toml.rs`**: `.crates.toml` 写入器 (`write_install_record` + 纯函数 `update_record`)。`cargo install --list` 实际读的是这个文件，downloader 路径之前只写 `.crates2.json` 导致升级后 `cargo install --list` 仍报旧版（用户在 cargo-fresh 第二次跑就会看到"Unchanged"假告警）
- **`crates2::lookup_bins(cargo_home, package_name)`**: 从 `.crates2.json` 提取 `bins[]` 数组。包名 ≠ binary 名 (ripgrep→rg, tauri-cli→cargo-tauri) 时 downloader 现在能正确解压 + 安装到 `~/.cargo/bin/<binary>`
- **`InstallSpec.bins`** 字段：caller (updater) 把 bins 列表传进下载器，`archive::extract` 接收 `&[String]` 候选并报告实际匹配上的 binary 名（写回 .crates2.json / .crates.toml 用）
- **`UpdatePlan`** (`src/updater/mod.rs`): rustup 风格行管理器。`new(names)` 一次性把所有选中包注册成对齐的 pending 行；`row(i)` / `name_width()` 给单个包的 update_package 拿到自己的行。0.12.0 并发调度器把外层循环换成 `JoinSet` 就能直接用同一份 API
- **`finalize_installed` / `finalize_failed` / `finalize_aborted`**: 把行定格成静态终态行（无 spinner / 无 bar），最右显示 `installed X.XX MiB`（downloader 路径）或纯 `installed`（cargo install 编译路径）
- **`InstallMethod` enum** + `UpdateResult::with_install_method`: 跟踪每包走的安装路径（Downloader/CargoInstall/Unknown），summary 末尾按方法分组通报 `Prebuilt: ...` / `Compiled: ...`
- **locale 键** `summary_prebuilt` / `summary_compiled` (EN + zh-CN)
- **`resolve.rs` 测试** `includes_tauri_style_subcrate_prefix_tag`: 锁住 monorepo tag 路径覆盖

### Changed

- **`resolve::candidate_urls(name, ...)` → `candidate_urls(name_candidates: &[String], ...)`**: `{name}` 占位符现在交叉乘 package 名 + binary 名（去重）。tauri-cli 这种文件名用 `cargo-tauri-*` 的现在能命中
- **tag 路径模板从 2 个扩到 6 个**: 通用 `v{version}` / `{version}` + monorepo 前缀 `{pkg}-v{version}` / `{pkg}-{version}` + 斜杠 `{pkg}/v{version}` / `{pkg}/{version}`。tauri-cli 的 `tauri-cli-v2.11.2/` 路径靠这个走通
- **HEAD 探测并发化** (`fetch::head_probe_concurrent`): `FuturesUnordered` + `Semaphore(16)` + 每 HEAD 5s 超时。"无 prebuilt" 判定从 6-20s（40-360 串行 × 150ms）降到 ~1-2s，最坏 5s
- **每包行 phase 文案反映当前阶段**: 不再用 `pb_status_dim` 把每个 phase 滚到上方（4 行 × N 包噪音），而是更新 spinner 的 prefix —— `resolving` / `[bar]` / `verifying` / `extracting` / `installing` 都在同一行原地切。`enable_steady_tick(80ms)` 让 spinner 真转
- **下载条改 rustup 风格**: `{name:>W} [bar] {bytes}/{total} ({%}) {speed} ETA {eta}`，名字按最长包名右对齐
- **回退到 cargo install 时行 prefix 切换到 `compiling from source`** (黄色加粗)：用户一眼看出哪些包要慢一截，对得上末尾 summary 的 `Compiled:` 分组
- **`update_package` 签名**: 新增 `row: Option<(ProgressBar, usize)>` 参数。`None` 走旧路径（自建独立 spinner，给老测试用）；`Some` 走新 rustup 风格行
- **summary 末尾分组**: success 列表后追加 `Prebuilt: a, b, c` (青) / `Compiled: d, e` (黄)，仅当对应分组非空
- **`install.rs` 同时写 `.crates2.json` 和 `.crates.toml`**: 两个文件都更新，不再"`cargo install --list` 报旧版"

### Removed

- **`src/ui/download_view.rs`** + 整个 `src/ui/` 模块：dormant 的 crossterm region 渲染器。`MultiProgress` 同时覆盖串行 (0.11.0) 和并发 (0.12.0) 两种场景，二套渲染器在 0.11.0 上线前就重复了
- **`crossterm` 依赖** (`Cargo.toml`)：只被刚删的 download_view.rs 用，可以一起摘
- **每包的 `Using 使用下载器` / `Using cargo install` 滚屏行**：summary 末尾按方法分组通报，行内不再赘述
- **`Updated <pkg> <旧> -> <新>` 滚屏行** (`verify_and_report_update`)：行末态 `installed X.XX MiB` 和 summary 已各报一次，第三次重复属于纯噪音

### Fixed

- **ripgrep / tauri-cli 走 fallback 的根因**: 不只是包名 != binary 名，monorepo tag 前缀也是必修；现在两者一起修了
- **`cargo-fresh` 二次运行报 Unchanged**: `.crates.toml` 写入修复后 `cargo install --list` 立即反映新版

### 下载器基础（原 0.11.0，2026-05-27）

替换 `cargo binstall` 子进程依赖为自实现的进程内二进制下载器。Crates 来源包的更新路径不再 spawn `cargo binstall`——`cargo-fresh` 自己 HTTP 流式下载 GitHub Release tarball、校验可选的 sha256 sidecar、原子化安装到 `~/.cargo/bin`、同步更新 `.crates2.json`。多 arch 命名变体探测覆盖 Rust triple / Go-style / Apple-short 三种约定，单个包最多 24 个候选 URL（Linux x86_64：3 约定 × 2 归档 × 4 别名）。临时目录全程由 `tempfile::TempDir` RAII 持有，安装结束或失败都不留 `/tmp` 残渣。BEHAVIOR：JSON `results[].phase` 仍是 `"binstall"`/`"install"` 字符串（schema 不变），但 `"binstall"` 现在意味着"downloader 路径成功"而非"`cargo binstall` 子进程成功"。

### Added

- **`src/downloader/` 模块**: 5 个子模块组成的完整下载流水线
  - `events.rs`: `ProgressEvent` (Resolving/UrlCandidate/Downloading/Verifying/Extracting/Installing/Done/Failed) + `DownloaderError` (Unsupported/Failed/Cancelled 三分法，决定调用方 fallback 路由) + `UnsupportedReason` / `FailureKind` 枚举
  - `resolve.rs`: 纯函数 `candidate_urls(name, version, repo, &[targets])` 生成 3 GitHub 约定 × 2 归档 × N 别名候选；`current_targets()` 返回当前平台的别名数组 (macOS aarch64 给 3 个，Linux x86_64 给 4 个)
  - `fetch.rs`: HEAD 探测每个候选，第一个 2xx 胜出；reqwest `bytes_stream()` 流式 GET，每 chunk 后发 `Downloading` 事件并检查 cancel；并发尝试 `{url}.sha256` (404 容忍, 匹配则强制校验, mismatch 立即失败)
  - `archive.rs`: tar.gz / zip / 裸二进制三路解压；`find_binary` BFS 深度 3 兼容"binary at root" (mdbook) 和"binary in subdir" (ripgrep) 两种 layout；`ExtractResult` 持 `TempDir`，Drop 时清理
  - `install.rs`: 拷贝到 `.cargo-fresh-{name}-{uuid}.tmp` + chmod 0o755 + fsync + atomic rename；rename 失败显式 `remove_file` 清 tmp
- **`src/package/crates_api.rs`**: 极小的 crates.io API client，只取 `crate.repository` 字段；任何错误返回 `None` 走 fallback
- **`src/ui/download_view.rs`**: crossterm 控制的多行 region 渲染器（10Hz tick、非 TTY 降级、Done/Failed 通过 println_above 滚入历史）。**0.11.0 暂未启用**——保留给 0.12.0 的并发调度器
- **多 arch 别名表 (`current_targets()`)**:
  - macOS aarch64: `aarch64-apple-darwin`, `arm64-apple-darwin`, `darwin-arm64`
  - macOS x86_64: `x86_64-apple-darwin`, `x64-apple-darwin`, `darwin-amd64`, `darwin-x64`
  - Linux aarch64: `aarch64-unknown-linux-gnu`, `aarch64-unknown-linux-musl`, `arm64-unknown-linux-gnu`, `linux-arm64`
  - Linux x86_64: `x86_64-unknown-linux-gnu`, `x86_64-unknown-linux-musl`, `linux-amd64`, `linux-x64`
- **`tests/downloader_fetch.rs`** (wiremock): 5 个集成测试覆盖 HEAD 404→200 切换 / 全部 HEAD 404 / sha256 mismatch / cancel-before-start / Downloading 事件顺序
- **`tests/downloader_install.rs`**: 隔离 `tempfile::TempDir` 作 `CARGO_HOME`，验证 atomic install + `.crates2.json` 写入
- **`tests/fixtures/mkfixtures.sh`** + 3 个 fixture 归档（ripgrep-like.tar.gz / mdbook-like.tar.gz / cargo-deny-like.zip）

### Changed

- **`update_package` 签名**: `cancel: &AtomicBool` → `cancel: Arc<AtomicBool>`。原 `&AtomicBool` 拷贝出去给 spawn 内的 downloader 时只能取快照，Ctrl-C 信号无法实时传入下载流——改 Arc 后修复
- **`build_args` 不再带 `use_binstall: bool` 参数**: 现在永远生成 `cargo install` 系列参数，binstall 分支彻底移除
- **`CommandSelector` 简化**: primary+fallback 仍保留（给将来其它 fallback 用），但 0.11.0 的 fallback 槽永远是 `None`——binstall→install 的粘性 fallback 不再需要
- **locale 字符串**: `using_binstall` 改为 "self-hosted downloader" / "使用下载器"；`binstall_failed_fallback` 文案改提"downloader"
- **`Cargo.toml` 新增 7 个直接依赖**: `flate2`, `tar`, `zip`, `sha2`, `crossterm`, `futures-util`, `tempfile`；`reqwest` 加 `stream` feature
- **依赖审计**: `cargo deny check licenses` 无新增 license（全部 MIT/Apache-2.0）

### Removed

- **`cargo binstall` 子进程路径**: `update_package` 的 binstall 分支整体删除。`is_binstall_available()` / `ensure_binstall_available()` / `install_binstall()` 仍存在但仅 `--check-binstall` 预检 flag 使用
- **测试 `binstall_command_includes_no_confirm` / `selector_sticks_to_fallback_across_retries`**: 被测代码路径已移除

### Deprecated

- **`--install-binstall` flag**: 保留接受但变为 no-op，首次使用时通过 `OnceLock` 打印一次弃用警告。计划 0.12.0 移除

### Notes for downstream

- JSON `schema_version=1` 不变。`results[].phase = "binstall"` 在 0.11.0 之后语义为"下载器路径"，而非"`cargo binstall` 子进程"
- 0.11.0 仍是串行更新——并发调度器作为单独的 0.12.0 规划，那时 `ui::download_view` 才会接入
- 下载器只支持 GitHub Release。非 github.com 域 / Windows / `package.metadata.binstall` 自定义模板都走 `Unsupported` 直接 fallback 到 `cargo install`，这是 MVP 边界——覆盖主流包 ~80%

## [0.10.7] - 2026-05-25

测试与文档加固型小版本——把 1.0 对外契约（JSON schema + cargo-style 输出行格式）钉进 CI，让"改了实现忘了改文档"和"顺手把 verb 改了一下"这类静默漂移在 PR diff 上无所遁形。无代码行为变化、无 BREAKING、无 BEHAVIOR。

### Added

- **JSON schema 校验测试 (`tests/json_schema.rs`)**: 用 `jsonschema` crate 把 `cargo_fresh::models::JsonReport` 的实际形状对着 `docs/json-schema.json` 跑校验。4 条 fixture 覆盖空快照 / 全字段满快照（每种 `$defs` 形状各一份）/ binstall 剩余枚举值（`source_build`、`unknown`）/ 反向 case（故意改坏 `format` 字段确认 validator 会报错，不是橡皮图章）。CI 上任何对 `JsonReport` 的字段增删如果忘了同步 schema 文件就直接 fail
- **状态行 snapshot 测试 (`tests/cli_snapshots.rs`)**: 用 `insta` 锁 8 条核心 verb 的行格式——`Fresh` / `Updating` (含 `[binstall: prebuilt]` 尾标版本) / `Skip [git]` / `Skip [unknown source]` / `Fallback` / `Failed` / `Finished`。ANSI 通过 insta filter 剥掉，`.snap` 文件是纯文本，跨 TTY / `CLICOLOR_FORCE` 环境稳定。任何对 verb 名 / 12 字符宽度 / 来源 marker 拼装的改动会在 PR diff 上以 `.snap` 变化形式出现，reviewer 一眼能看见

### Changed

- **`src/display/mod.rs` 提取 `format_status_line(verb, msg, style)` 纯函数**: 四个 `status*` + 四个 `pb_status*` 函数从各自重复一遍 `format!("{} {}", colored_padded_verb, msg)` 改为全部 delegate。少 ~24 行重复，行为完全等价（snapshot 测试覆盖）。`pub enum StatusStyle { Ok, Warn, Err, Dim }` 为契约层提供单一 render 路径；`package_transition` 改为 `pub` 以便测试直接调

### Docs

- **CLAUDE.md 同步到 0.10.6**: 测试计数从 `110 unit + 18 integration as of 0.10.3` 更新为 `123 unit + 18 integration as of 0.10.6`；新增 `src/package/binstall_probe.rs` 的模块表行；JSON_MODE 设计决策段补全 0.10.3+ 的 `skipped[].reason_code` / `version_check_errors[]` / `summary.selected`/`attempted`/`check_errors` 与 0.10.4 的 `updates_available[].binstall`；新增 `--check-binstall` 预检的设计决策段；Roadmap 状态从 v0.10.3 推到 v0.10.6，补全 0.10.4 / 0.10.5 / 0.10.6 三段（`--check-binstall`、binstall 交互挂死修复、Ctrl-C 取消修复、`man` 自动分页、fish completion hint、CHANGELOG-sourced release body）

## [0.10.6] - 2026-05-24

发布流程小修：GitHub Release 页面现在显示当前版本对应的 CHANGELOG 章节，不再是那段千篇一律的 "This release includes the latest updates" 模板；新增 CI `changelog-sync` job，`Cargo.toml` bump 时若没把 `[Unreleased]` 内容搬到对应版本节就直接 fail，避免 tag push 后才发现。无代码变更、无 BREAKING、无 BEHAVIOR。

### Changed

- **GitHub Release body 改为从 `CHANGELOG.md` 抽取**: `crate.yml` 用 awk 切出 `## [X.Y.Z]` 到下一个 `## [` 之间的内容写入 release body，附上指向上一 tag 的 `Full Changelog` compare 链接。抽不到对应章节硬 fail——空 body 比废话 body 难看，但比悄悄回退到模板可控。`actions/checkout@v5` 加 `fetch-depth: 0` 以便能看到全部历史 tag

### Added

- **CI `changelog-sync` job**: 在 `ci.yml` 里加一个独立 job，校验 `Cargo.toml` 当前 version 在 `CHANGELOG.md` 里必须有 `## [VERSION]` 章节。PR 阶段就把"忘了写 changelog 就 tag 了"挡掉，不让 `release.yml` 的 hard-fail 出现在 tag push 的红字邮件里
- **CONTRIBUTING 注明 release body 来源**: 显式写出 GitHub Release body 由 CHANGELOG 对应章节生成，提醒维护者把每个版本节当成对用户的发版说明来写

## [0.10.5] - 2026-05-24

修复型小版本：`cargo fresh man` 在 TTY 下自动用系统 `man` 渲染分页；fish `--cargo-fresh` 补全生成时打印安装路径 hint，避开 `cargo-fresh.fish` 文件名陷阱。无 BREAKING、无 BEHAVIOR。

### Fixed

- **`cargo fresh man` 在 TTY 下自动用 `man` 渲染并分页**: 之前总是把 raw roff 直接 dump 到 stdout，交互式终端里完全不可读。现在 stdout 是 TTY 时，写到临时文件并调用系统 `man <tmpfile>`——让 `man` 处理排版、分页、宽度。stdout 被重定向/管道时仍输出 raw roff，`cargo fresh man > cargo-fresh.1` 与 `cargo fresh man | mandoc` 这条路径不变
- **fish `--cargo-fresh` 补全生成时打印安装提示**: `~/.config/fish/completions/cargo-fresh.fish` 这个文件名只在输入 `cargo-fresh<TAB>` 时被 fish 自动加载，**不会**响应 `cargo fresh<TAB>`——是 `--cargo-fresh` 这个 flag 上手时最常踩的坑。stdout 重定向、stderr 是 TTY 时，命令尾打一行 hint 指向正确路径（`~/.config/fish/completions/cargo.fish` 或 `~/.config/fish/conf.d/cargo-fresh.fish`）。README 同步更新

## [0.10.4] - 2026-05-22

修复型小版本:根因修复 `cargo binstall` 更新挂死在交互确认提示上的问题;新增 `--check-binstall` 检查阶段预检;修正 Ctrl-C 取消被误报成更新失败。无 BREAKING、无 BEHAVIOR——可放心从 0.10.3 升级。

### Added

- **`--check-binstall` —— 检查阶段的 binstall 预检**: 加上这个 flag 后,cargo-fresh 在检查阶段对每个更新候选并发跑 `cargo binstall --dry-run`,在 `Updating` 行尾标出这次升级会拿到预编译二进制(`[binstall: prebuilt]`,绿)还是退化成从源码构建(`[binstall: source build]`,黄)。专治"crate 刚发版、crates.io 已有新版但 GitHub release 二进制还没传完"那段窗口期——此时 binstall 会闷头编译十几分钟,预检能在启动更新前就把它标出来。默认关闭(每个候选的 dry-run 约 10s,会 spawn cargo 并联网,已并发执行);binstall 未安装时打一行 Hint 并照常继续。`--format=json` 的 `updates_available[]` 同步新增 `binstall` 字段(`prebuilt` / `source_build` / `unknown` / `null`,`schema_version=1` 增量)

### Fixed

- **`cargo binstall` 更新不再挂死在交互确认提示上**: `cargo binstall` 默认会打印 `Do you wish to continue? [yes]/no` 并阻塞读 stdin。cargo-fresh 用管道捕获 binstall 的 stdout/stderr——提示文字被吞进管道、用户看不见;binstall 又继承了 cargo-fresh 的 TTY stdin,于是死等一个用户根本不知道要回答的 "yes",整个更新无声挂死。`Slow` watchdog 此前把这种挂起误报成"从源码构建",`--check-binstall` 也能查到预编译产物却照样挂——因为问题根本不是源码构建。现在 binstall 命令恒带 `--no-confirm`;另外 `run_cargo` 把子进程 stdin 接到 `/dev/null` 作为第二层防线,任何 cargo 子命令都无法再卡在交互提示上
- **Ctrl-C 不再被误报成更新失败**: 之前 `update_package` 不接收取消标志,`cancel` 只在 `main` 的包循环顶层检查一次。用户按 Ctrl-C 中断一个慢更新时,同进程组的 SIGINT 会顺带杀死 cargo 子进程(`status.code()` 变成 `None`,显示为 `exit code: -1`),旧逻辑把它当成普通命令失败,触发 binstall→install 回退、再重试 3 次,最后在总结里把这个包标成"失败"。现在取消标志贯穿 `update_package` 的整个重试循环:命中即立即停手——不回退、不重试,该包标为 `Aborted` 而非 `Failed`,也不计入失败数

## [0.10.3] - 2026-05-22

收尾型小版本：补全 `.crates2.json` 安装选项保留、修正 binstall 回退重试逻辑、扩充 `--format=json` 的机器可读字段。含一处 BEHAVIOR——带非默认 features 的包更新时跳过 binstall，无自定义 features 的包行为不变。

### BEHAVIOR

- **带非默认 features 的包跳过 binstall**: 一个包若以 `--features` / `--no-default-features` / `--all-features` 安装,更新时直接走 `cargo install`(binstall 下的是上游预编译二进制,无法应用任意 features)。无自定义 features 的包行为不变,仍优先 binstall

### Added

- **`--format=json` 新增 `skipped[].reason_code`**: 稳定的机器可读枚举(`path_source` / `git_source` / `unknown_source`),脚本可据此分支,不必再解析人类可读的 `reason` 文案
- **`--format=json` 新增 `version_check_errors[]`**: crates.io 包的最新版本查询失败时记录于此,每项带 `name`、`kind`(`not_found` / `unavailable`)和人类可读的 `error`。`fresh[]` 现在会排除这些包——此前查询失败的包会被悄悄当作"已是最新"混入 `fresh[]`,导致空的 `updates_available` 不可信。CI 可据此区分"无更新"与"无法确认是否最新"
- **`--format=json` 的 `summary` 新增 `selected` / `attempted` / `check_errors` 计数**: 分别为本次选中更新的包数、实际执行了安装命令的包数、以及 `version_check_errors[]` 的长度。自动化可据此区分"有更新但策略未应用"与"无可操作项"
- **`.crates2.json` 安装选项保留**: 新增 `src/package/crates2.rs`,从 `$CARGO_HOME/.crates2.json` 解析每个包安装时的 features 选项,更新时透传给 `cargo install`。尽力而为——文件缺失/损坏/无匹配条目一律静默回退默认行为,绝不让它成为更新失败的原因

### Fixed

- **binstall 回退后不再跑回 binstall**: 之前重试循环始终重跑主命令——binstall 失败、回退到 `cargo install` 也失败后,第 2/3 次重试又跑回 `cargo binstall`。binstall 一旦在当前环境失败(典型是无预编译产物、退化成从源码构建后仍失败),重试它只会重复那条又慢又必然失败的路径。现在引入 `CommandSelector`:首次回退后把 `cargo install` 锁定为后续每一次重试的命令,worst-case 命令序列从 `binstall, install, binstall, binstall` 修正为 `binstall, install, install, install`
- **更新不再静默丢 features**: 之前 `cargo install --force <name>` 不带任何 `--features`,把以自定义特性安装的包(如 `ripgrep --features pcre2`)悄悄退回默认特性。现在从 `.crates2.json` 还原并保留 `--features` / `--no-default-features` / `--all-features`

### Docs

- **README 对比表纠偏**: 之前对 cargo-update 的描述有三处与事实不符——它支持 git 包(非 "Crates only")、包选择是精确名/`--all`(非 "Substring match")、binstall 可用时会自动启用(非 "N/A")。新表如实陈述,并新增 "Install options preserved" 一行坦白 cargo-fresh 目前会把 `--features` 重置为默认(1.0 前修复)

## [0.10.2] - 2026-05-19

打磨型小版本，全是为 1.0 合约清理边角。无 BREAKING、无 BEHAVIOR——可放心从 0.10.1 升级。

### Added

- **stdout / stderr 分流明确化**: 所有 `status*` / 进度条 / prompt 经 `anstream::eprintln!` 走 stderr；`--format=json` 的报告独占 stdout 一行。下游 `cargo fresh --format=json | jq` 不再需要任何过滤，`cargo fresh > /dev/null` 仍然看得到进度。两条回归测试钉合约
- **`docs/json-schema.json`**: JSON Schema Draft 2020-12 描述 `JsonReport schema_version=1` 的完整字段形状。1.x 内只新增字段、不重命名/删除。README 加 "Output streams" + jq 用例小节
- **`cargo fresh man`**: 用 `clap_mangen` 把同一份 `Cli::command()` 渲染成 roff 到 stdout。`cargo fresh man > ~/.local/share/man/man1/cargo-fresh.1` 后 `man cargo-fresh` 走系统 man 索引。镜像 `completion` 子命令的设计，不接触发布流程、不需要 build.rs
- **`audit.yml` CI workflow**: 跑 `cargo-deny check advisories licenses sources bans` + `cargo-audit`。触发面是 manifest / Cargo.lock / deny.toml 改动的 push & PR，外加每周一 06:00 UTC cron——新出现的 RustSec 公告能被定期扫到。`deny.toml` 的 license allowlist 基于 `cargo license` 实盘点

### Changed

- **颜色管线接 `anstream`**: `colored` 继续提供 `.green().bold()` 的人体工学 API，但是否真的输出 ANSI 由 `anstream::AutoStream::choice(&stderr)` 一处决定后下发给 `colored::control::set_override`。`NO_COLOR` / `CLICOLOR_FORCE` / `TERM=dumb` / TTY 检测全部经一处，不再两套逻辑互相打架。两条回归测试覆盖 `NO_COLOR=1` 必须 ANSI-free、`CLICOLOR_FORCE=1` 即便 stderr 不是 TTY 也保留颜色码

### Tests

- 集成测试 `tests/cli.rs` 从 5 条扩到 10 条：`json_mode_keeps_stdout_clean` / `non_json_mode_keeps_status_off_stdout` / `man_subcommand_emits_roff` / `no_color_env_strips_ansi_from_stderr` / `clicolor_force_keeps_ansi_when_redirected`——把 1.0 对外契约的几条关键合约都钉在 CI 上

## [0.10.1] - 2026-05-18

### BEHAVIOR

- **`cargo fresh` 不再静默安装 cargo-binstall**: 之前发现 binstall 不可用时会自动 `cargo install cargo-binstall`——CI 与受控环境用户表示这种"悄悄改 toolchain"令人不安。新版本默认只打 Hint，走 `cargo install` 慢路径；需要自动安装可显式加 `--install-binstall`。Dry-run 永远不动 toolchain

### Added

- **`--install-binstall`**: 显式启用"binstall 不可用时自动安装"的旧行为
- **`--no-cargo-search-fallback` / `CARGO_FRESH_NO_FALLBACK=1`**: sparse index 失败后跳过 `cargo search` 兜底——私有 registry / 离线沙箱 / 镜像配置错误的诊断利器
- **非 TTY 自动降级**: stderr 不是 terminal（CI 日志、管道、`tee`）时禁用 spinner，`pb.println` 仍正常输出。JSON 模式同样走这条路径

### Changed

- **`--verbose` 输出统一走 `status_*`**: `check_package_updates` 三处裸 `println!` 改成 `status_dim` / `status_warn`，新增 `Check` / `Latest` 两个动词头
- **cargo 子调用全部走 `tokio::process::Command`**: `get_installed_packages` / `get_installed_version` / `cargo_search_fallback` / `install_binstall` / `run_cargo` 不再阻塞 tokio runtime；`is_binstall_available` 因 `OnceLock` 缓存最多调用一次而保留 sync 实现
- **tokio features 收紧**: `full` → `[macros, rt-multi-thread, signal, process, time, sync]`，依赖体积下降
- **MSRV 抬到 1.86**: 1.82 已无法 `cargo check --locked`——`clap_derive 4.6.1` 要求 `edition2024`（Rust 1.85 稳定），`icu_*@2.2.0`（reqwest → url → idna → icu）声明 `rust-version = 1.86`。一口气抬到 1.86 避免下一个 patch 又要再调一次。CI MSRV job 同步改成 1.86.0

### Added

- **可执行错误提示 (`errors::hint_for`)**: 用 `thiserror` 建模 `CargoFreshError::CargoListFailed`，外加 `reqwest::Error` 网络层匹配。`main` 拿到错误时在 stderr 多打一行 `Hint`，给出"`cargo --version` 验证 toolchain"或"设置 HTTPS_PROXY"这类**具体可执行**的下一步操作

### Fixed

- **`INSTALLED_VERSION_CACHE` 写入不再静默丢失**: 改用 `OnceLock::get_or_init` + `lock/clear/extend`，未来 `--watch` 多次扫描能正确刷新缓存。`invalidate_installed_version` 单条移除语义保持
- **locale 检测的并发竞争**: 抽出纯函数 `detect_from_locale(&str)`，测试不再 `env::set_var`；`cargo test` 可放心并发跑，CI 撤掉 `--test-threads=1`

### Tests

- **新增 `tests/` 集成测试**: `tests/cli.rs` 用 `assert_cmd` 跑 `--version` / `--help` / `completion {bash,fish}` 的对外契约；`tests/sparse_index_http.rs` 用 `wiremock` 覆盖 sparse index 客户端的 200 / 404 / 5xx / 重试恢复 / 空 body 五种路径，全程不联网
- **暴露 `src/lib.rs`**: bin 与 lib 共用同一份模块树，集成测试可直接 `cargo_fresh::package::sparse_index::fetch_latest`

## [0.10.0] - 2026-05-18

### BREAKING

- **`--include-prerelease` 现在真正生效**: 旧版本即使不加这个 flag，`check_package_updates` 也会把更新的预发布版本写入 `latest_version`，触发交互选择里的预发布分组。0.10.0 起严格遵守 flag 语义——不加 `--include-prerelease` 看不到任何预发布候选。如果你过去依赖"不加 flag 也能看到 rc"的隐式行为，请显式传 `--include-prerelease`

### Added

- **`--registry-url URL`**: 显式覆盖 sparse index 的 base URL（如 `https://mirrors.ustc.edu.cn/crates.io-index/`）
- **自动识别 cargo 镜像配置**: 解析 `$CARGO_HOME/config.toml` 的 `[source.crates-io] replace-with` → `[source.<name>].registry`（仅支持 `sparse+URL` 前缀），命中后所有版本检查走镜像，无需配置 `--registry-url`。git registry 镜像继续走 `cargo search` 兜底
- **Ctrl-C 取消**: 更新循环响应 SIGINT。命中后跳过剩余包，打印 `   Aborted N/M completed`，以退出码 130 退出。子进程内的取消（cargo install 已在跑）属于 P1 范畴
- **`--format=json` 与稳定退出码契约**: CI / 脚本友好的输出模式。JSON 模式禁用所有彩色、spinner 和 dialoguer 交互，结尾在 stdout 打一行 `JsonReport`（schema_version=1）。退出码：`0` 全部成功、`1` 有可更新但未应用、`2` 至少一个失败、`130` 用户取消。README 加 "Exit Codes" 一节

### Changed

- **`CommandFactory` 派生补全脚本**: 消除了 `cli/mod.rs` 里手写的第二份 `clap::Command`，补全脚本现在永远跟真实 CLI 同步
- **新增 `package/registry.rs`**: 7 个 unit test 覆盖 ustc 风格 mirror、git mirror、缺失配置、URL 规范化
- **`choose_latest` 抽成纯函数**: 选择逻辑从 `check_package_updates` 里拆出，7 个 unit test 覆盖 `--include-prerelease` on/off × stable/pre 在/不在 的矩阵
- **声明 MSRV (`rust-version = "1.82"`)**: 当前代码用到 `Option::is_none_or`（1.82 稳定），明确写到 `Cargo.toml`。新增 `.github/workflows/ci.yml` 同时跑 stable 与 1.82 toolchain 的 `cargo check`，避免 MSRV 在维护中静默漂移

## [0.9.14] - 2026-05-17

### Fixed
- **进度条残留污染输出**: 旧版同时活着两个进度条——`create_main_progress_bar` 的 `0/1 cargo-fresh (1/1)` 总进度条 + 每包 spinner——两者在终端互相覆盖，且 spinner 在 `update_package` 多条 return 分支漏写 `finish_and_clear`，留下 `⠋` 帧残留污染后续输出。修复后输出干净

### Changed
- **删除主进度条**: `create_main_progress_bar` 整个函数移除。包数 N/M 提示改用单行 `   Package 3/18 cargo-fresh` 状态行，仅在多包升级时显示
- **引入 `PbGuard` RAII 守卫**: `update_package` 持有 spinner pb 后立刻包进守卫，Drop 时自动 `finish_and_clear()`，保证从任何 return 分支（成功 / 失败 / 重试用尽 / dry-run）退出都不会留下 spinner 帧
- 删除 `models::PROGRESS_BAR_WIDTH` 常量（主进度条移除后已无人使用）

### Technical
- 全部 63 个单元测试通过；`cargo clippy --all-targets -- -D warnings` 零警告
- 实测验证：本机降级到 0.9.11 再用 0.9.14 二进制升回 0.9.13，输出无 spinner 残留

## [0.9.13] - 2026-05-17

### Changed
- **CLI 输出重做为 cargo 风格**: 全面剥离 emoji（✅❌⚠️📋🧪🔍⚡🔄📦），改用 cargo 自身的 `   Verb message` 风格——12 字符右对齐绿色加粗动词 + 内容。视觉风格与 `cargo build` / `rustup` 一致，更专业
- **多行展示压缩为单行**: 旧版每个升级包要 3 行（`xxx 有更新可用\n  当前版本: 0.9.8\n  最新版本: 0.9.10`），现在一行搞定（`    Updating cargo-fresh 0.9.8 -> 0.9.12`）
- **统一的状态动词词典**: `Checking` / `Found` / `Updating` / `Updated` / `Fresh` / `Running` / `Installing` / `Installed` / `Would run` / `Fallback` / `Unchanged` / `Failed` / `Note` / `Finished` 等。绿色（成功）/ 黄色（警告）/ 红色（失败）/ dim 灰（次要信息）四种语义颜色
- **摘要尾行合并**: 旧版三行 `✅ Update completed!\n成功: 1 个包\n总耗时: 63 毫秒` 压缩为 cargo 风格单行 `    Finished 1 个成功, 耗时 63ms`

### Added
- **`display::status*` 系列辅助函数**: `status` / `status_warn` / `status_err` / `status_dim` 用于直接 println；`pb_status*` 四个对应版本用于进度条上下文。所有用户面输出都走这 8 个函数，保证视觉一致性

### Fixed
- **`no_updates_selected` 键名拼写错误**: 老代码引用的 key 不存在，导致 `--no-interactive` 走完后那行提示永远是空字符串。改成正确的 `no_packages_selected`

### Technical
- `cargo install --list` 解析的箭头从 `→` 改为 ASCII `->`，避免 mono 字体下宽度估算不一致
- 4 个 `format_text` 单元测试的硬编码断言更新以匹配新模板（断言文本去除 emoji 前缀）
- 全部 63 个单元测试通过，`cargo clippy --all-targets -- -D warnings` 零警告

## [0.9.12] - 2026-05-17

### Added
- **新模块 `package::sparse_index`**: crates.io sparse index 客户端，作为 `cargo search` 的高速替代。直接 HTTPS GET `https://index.crates.io/{shard}/{name}`，按行 JSON 解析、按 semver 取最新未 yank 版本，同时返回稳定 + 预发布两个候选。`fetch_latest` 异步函数 + `parse_index_body` 纯函数 + `index_path` 分片规则函数三层分离，便于离线单测
- **`fetch_latest_versions` 统一入口**: 主路径走 sparse index；网络错误、HTTP 非 2xx、解析失败时自动回退到 `cargo search`。回退路径无法一次拿两个版本，按 `include_prerelease` 标志选一个填入对应字段

### Changed
- **`check_package_updates` 改单次 RPC 同时拿稳定+预发布**: 旧版稳定版和预发布版各发一轮请求；新版 sparse index 单次响应即包含全部历史版本。`main.rs` 删除了串行 prerelease 循环（19 行）
- **并发限流 `Semaphore(16)`**: `check_package_updates` 中每个 `tokio::spawn` 先 acquire permit 再发请求，防止超大包数（100+）触发 crates.io 限流或本地 fd 耗尽
- **缓存 `cargo install --list` 结果**: 新增 `INSTALLED_VERSION_CACHE`（`OnceLock<Mutex<HashMap>>`），`get_installed_packages` 首次解析时填充；`get_installed_version` 优先读缓存，避免 N 个包升级需要 N+1 次 `cargo install --list` 启动 cargo 子进程。升级成功后通过 `invalidate_installed_version` 失效单条记录强制下次重读真实状态

### Technical
- 引入 `reqwest = "0.12"` 依赖，`default-features = false` + `rustls-tls` 避免拉 native-tls / openssl 链
- 单进程共享 `OnceLock<reqwest::Client>` 复用 connection pool，UA 设置为 `cargo-fresh/VERSION`，超时 10s
- 新增 11 个单元测试覆盖 sparse index：`index_path`（1/2/3/4+ 字符分片、大小写归一）共 5 个；`parse_index_body`（最大稳定版、稳定+预发布分流、跳过 yank、跳过不可解析行、空输入、全 yank）共 6 个。测试数 52 → 63
- 实测性能：本机 18 个全局包，沿用 0.9.11 二进制需要 ~22s，新二进制 ~2.6s，瓶颈现在是纯网络延迟（单次 sparse index RPC ~1s，Semaphore(16) 两个 wave 完成）

### Notes
- `cargo search` 回退路径完整保留——为企业代理 / 防火墙环境留好生路
- 未做：完整 `std::process::Command` → `tokio::process::Command` 重写。剩余 `Command` 调用只在 sequential update loop 中执行（一次一个包），不阻塞并发热点；sparse index 接入后真正的瓶颈已消除。完整重写收益小、破坏 binstall 探测逻辑的风险大，推到 1.0.0-rc.1 阶段

## [0.9.11] - 2026-05-17

### Added
- **真正的 glob 过滤**: `--filter` 改用 `globset` crate，支持标准 glob 语义（`*` / `?` / `[abc]`）。无 glob 字符的纯词自动包裹为 `*p*` 保留旧版"模糊匹配"友好行为
- **`--exclude PATTERN`**（可重复）: 从过滤后的列表再剔除匹配的包。先 filter 后 exclude，支持完整 glob 语法
- **`--dry-run`**: 打印将要执行的 cargo 命令但不实际执行。绕过进度条直接 println 保证命令清晰可见；连 binstall 安装副作用都避免（用只读 `is_binstall_available` 探测而非 `ensure_binstall_available`）
- **支持 git 和 path 安装源**: 解析 `cargo install --list` 的 `(git+URL#rev)` 和 `(path+file:///DIR)` 后缀，在显示时附加 `[git]` / `[path]` 标记。更新策略按来源分流：crates.io 走 binstall→install fallback；git 用 `cargo install --git URL [--rev REV] --force`；path 用 `cargo install --path DIR --force`

### Fixed
- **`parse_package_line` 误切 git URL**: 旧实现用 `split(':').next()` 找版本字段边界，但 git URL 中的 `https://` 含 `:`，会被错误截断成 `url = "https"`。改用先 `strip_suffix(':')` 剥掉行尾冒号再 `split_once(" v")`

### Changed
- 引入 `globset = "0.4"` 依赖
- `PackageInfo` 加 `source: PackageSource` 字段，新增 `PackageInfo::with_source` 构造器
- `check_package_updates` 跳过非 crates.io 源（git/path 在 crates.io 上查不到"最新版本"）
- `update_package` 签名扩展为 `(name, version, source, dry_run)`

### Technical
- 新增 9 个单元测试（52 总数）：3 个 PackageSource 解析（registry / git±rev / path）、2 个 glob 过滤（prefix / suffix）、3 个 exclude（空列表 / 单模式 / 多模式）、原有 7 个 parse_package_line 测试更新为新签名
- `cargo clippy --all-targets -- -D warnings` 零警告
- 实测验证：`--filter cargo-fresh --dry-run --batch` 在 100ms 内打印 `cargo binstall --force cargo-fresh --version 0.9.10` 命令且未修改本地安装

## [0.9.10] - 2026-05-17

### Fixed
- **yank 回滚场景误报需要更新**: `PackageInfo::has_update` 旧实现用字符串 `!=` 比较，当本地版本高于 crates.io 最新版本（例如上游 yank 后回滚）时会被误报需要更新。现在改用 `semver::Version` 比较，仅在 `latest > current` 时返回 true
- **含 "rc" 字面量的稳定版被误判为预发布**: `is_stable_version` / `PackageInfo::is_prerelease` 旧实现用 `contains("rc")`，会把 `1.0.0+rc-meta`、`1.0.0+arc-build` 等含子串 "rc" 的合法稳定版误判为预发布。现在改用 semver 标准 `Version.pre.is_empty()` 判断

### Changed
- **引入 `semver = "1"` 依赖**: 用 semver crate 替代字符串关键字匹配做版本判断和预发布检测
- **删除 `PRERELEASE_KEYWORDS` 常量**: 改用 semver 标准 API
- **`Cargo.lock` 入库**: 二进制 crate 必须提交 `Cargo.lock` 以保证可复现构建。从 `.gitignore` 移除

### Technical
- 新增 29 个单元测试覆盖核心纯函数：`parse_package_line`（7 个）、`extract_version_from_line`（3 个）、`is_stable_version`（4 个含关键回归）、`filter_packages`（4 个）、`PackageInfo::has_update`（8 个含 yank 回滚、major upgrade、build metadata、不可解析字串等场景）、`PackageInfo::is_prerelease`（3 个）
- 测试数从 14 增加到 43，`cargo clippy --all-targets -- -D warnings` 零警告
- 关于 semver crate 的 `Ord` 实现：会比较 build metadata 提供全序（虽然 SemVer 规范说应忽略）。对 cargo-fresh 来说这恰好对路——同语义版本但 build 不同通常意味着上游重发了 artifact，值得 `cargo install`

## [0.9.9] - 2026-05-17

### Fixed
- **i18n 多占位符模板渲染 bug**: 修复 `package_updated_version`、`package_update_failed`、`package_error`、`retry_attempt` 等模板的渲染问题。旧代码使用链式 `.replace("{}", x).replace("{}", y)`，第一次调用就会替换掉模板中所有 `{}`，导致第二、第三个变量永远不显示
- **`retry_attempt` 模板未生效**: 原实现 `.replace("{}", "").trim()` 抹空模板再手动拼接，现在能正确显示 "Retry attempt N for X..."
- **进度条生命周期**: 不再在 `pb.finish_and_clear()` 后继续使用进度条，改用 `enable_steady_tick` / `disable_steady_tick` 配对

### Changed
- **新增 `Language::format_text(key, &[(name, value)])`**: 使用命名占位符（`{name}` / `{old}` / `{new}` / `{code}` / `{error}` / `{attempt}`）替代位置占位符，每个变量只替换到自己的位置
- **`updater::update_package` 重构去重**: 抽出 `build_args` / `run_cargo` / `verify_and_report_update` / `report_command_failure` 四个辅助函数，消除 binstall → install 回退路径中约 80 行的"验证安装结果 + 打印 + 返回 UpdateResult"重复代码。`src/updater/mod.rs` 从 355 行降到 219 行（-38%）
- **`executing_command` 日志改为每次重试都打印**: 便于排查重试时实际执行的命令

### Technical
- 新增 4 个 `Language::format_text` 单元测试，关键回归测试锁死 `package_updated_version` 三个变量不再串扰
- 删除未使用的 `package_updated` 文本键
- 全部 14 个单元测试通过，`cargo clippy -- -D warnings` 零警告

## [0.9.8] - 2025-10-18

### Fixed
- **cargo binstall 缓存逻辑修复**: 修复了 cargo binstall 检查逻辑中的命令参数错误
- **重复安装提示问题**: 解决了即使 cargo binstall 已安装仍显示"正在安装 cargo binstall"的问题
- **缓存机制优化**: 改进了 `is_binstall_available()` 函数，使用正确的命令检查可用性

### Enhanced
- **用户体验优化**: 避免重复的安装提示，提供更清晰的状态反馈
- **性能提升**: 避免不必要的系统调用和重复操作
- **缓存效率**: 确保 cargo binstall 状态只检查一次，提升响应速度

### Technical
- 修复 `is_binstall_available()` 函数中的命令参数错误（`--version` → `--help`）
- 优化 `ensure_binstall_available()` 函数的逻辑流程
- 改进缓存机制，确保正确的状态管理
- 提升 cargo binstall 集成稳定性和可靠性

## [0.9.7] - 2025-10-18

### Added
- **智能缓存机制**: 添加 cargo binstall 状态缓存，避免重复检查和安装
- **时间统计功能**: 显示更新过程的总耗时，提供性能反馈
- **优化的进度条**: 改进进度条样式和显示逻辑，提供更好的视觉反馈

### Enhanced
- **用户体验**: 修复 cargo binstall 重复检查问题，提供更流畅的安装体验
- **进度显示**: 优化进度条显示，使用更美观的样式和清晰的状态提示
- **状态反馈**: 添加表情符号和更清晰的操作提示，提升用户交互体验

### Technical
- 新增 `OnceLock` 缓存机制，避免重复检查 cargo binstall
- 实现时间统计功能，使用 `std::time::Instant` 记录更新耗时
- 优化进度条样式，添加旋转器和改进的完成状态显示
- 改进状态反馈信息，统一视觉风格和用户体验

## [0.9.6] - 2025-10-18

### Added
- **快速安装支持**: 集成 `cargo binstall` 支持，提供更快的包安装体验
- **自动补全完善**: 支持 zsh、bash、fish、nushell 的自动补全功能
- **代码质量优化**: 清理未使用代码，修复所有 Clippy 警告

### Enhanced
- **安装体验**: 使用 `cargo binstall` 进行快速安装，支持自动回退到 `cargo install`
- **补全功能**: 完善 shell 补全脚本，支持 `cargo fresh` 和 `cargo-fresh` 两种调用方式
- **代码质量**: 零编译警告，零 Clippy 警告，符合 Rust 最佳实践

### Technical
- 新增 `cargo binstall` 集成，支持快速包安装
- 更新补全脚本生成逻辑，支持多种 shell
- 清理未使用的 `error_handling` 和 `http_client` 模块
- 移除未使用的依赖项 `lazy_static` 和 `reqwest`
- 修复所有 Clippy 警告，提升代码质量

## [0.9.5] - 2025-10-18

### Added
- **并发处理**: 使用 `tokio::spawn` 实现并发包检查，性能提升 3-5 倍
- **批量操作**: 新增 `--batch` 选项，支持自动更新所有包而无需确认
- **包过滤**: 新增 `--filter` 选项，支持按名称模式过滤包（支持通配符）
- **HTTP 优化**: 实现连接池和请求缓存机制，提升网络请求性能
- **增强错误处理**: 智能重试机制，指数退避策略和用户友好的错误消息
- **性能优化**: 并发包检查、HTTP 连接复用、请求缓存等多项性能改进

### Enhanced
- **用户体验**: 更详细的进度显示和状态指示
- **错误处理**: 区分不同类型的错误并提供相应的处理策略
- **网络稳定性**: 增强网络重试机制和离线模式支持
- **文档更新**: 全面更新 README 文档，添加新功能说明和使用示例

### Technical
- 新增 `src/http_client/mod.rs` 模块，实现 HTTP 客户端优化
- 新增 `src/error_handling/mod.rs` 模块，实现增强的错误处理
- 更新 `src/package/mod.rs`，实现并发包检查和过滤功能
- 更新 `src/cli/mod.rs`，添加新的命令行选项
- 更新 `src/main.rs`，集成新功能和优化逻辑

## [0.9.4] - 2025-10-16

### Fixed
- 修复重复的 `[Y/n]: [Y/n]` 提示问题，移除文本中的重复提示符
- 修复 `dialoguer` 库在非终端环境中的错误处理
- 优化更新完成后的信息显示，移除重复的成功信息
- 修复 `src/locale/texts.rs` 中的语法错误（缺失逗号）

### Enhanced
- 改进交互式确认的用户体验，使用 `show_default(false)` 配置
- 优化错误处理机制，在非终端环境中优雅降级
- 完善 GitHub Actions 工作流配置，修复项目名称不匹配问题
- 添加自动化的 crates.io 发布和 GitHub Release 创建流程

### Changed
- 更新 GitHub Actions 工作流，支持自动触发 release 构建
- 修复 Homebrew formula 配置，指向正确的项目仓库
- 优化 release 流程，实现推送标签后自动发布到 crates.io 并创建 release

## [0.9.3] - 2025-10-15

### Enhanced
- 完善updater模块的国际化支持，所有更新相关文本支持中英文切换
- 完善package模块的国际化支持，所有包检查相关文本支持中英文切换
- 添加17个新的文本键，确保完整的双语支持
- 优化语言检测测试，确保环境变量正确恢复

### Fixed
- 修复语言检测测试中的环境变量污染问题
- 修复文本键重复定义问题

## [0.9.0] - 2025-10-15

### Major Release

- 项目重命名为 `cargo-fresh`，支持 `cargo fresh` 子命令
- 添加自动语言检测功能，根据系统语言环境自动选择输出语言
- 支持中英文双语界面，提升国际化用户体验

### Added

- 添加自动语言检测功能，检测系统环境变量 (LANG, LC_ALL, LC_CTYPE)
- 支持中文环境自动显示中文界面
- 支持英文环境自动显示英文界面
- 创建 locale.rs 模块处理多语言支持
- 实现完整的中英文文本映射系统
- 支持所有用户界面文本的多语言显示
- 添加 Language 枚举类型 (English/Chinese)
- 为所有输出函数添加多语言参数支持

### Changed

- 项目名称从 `pkg-checker` 重命名为 `cargo-fresh`
- 支持 `cargo fresh` 子命令调用方式
- 更新 Cargo.toml 配置支持 cargo 子命令
- 修改主程序支持 cargo 子命令参数处理
- 更新所有文档和示例使用新的命令名称
- 重构 display 模块支持多语言输出
- 优化用户体验，根据系统语言自动选择界面语言

### Fixed

- 修复语言检测的环境变量处理逻辑
- 改进多语言文本的生命周期管理
- 优化编译警告，清理未使用的导入

## [0.8.1] - 2025-10-08

### Added

- 添加 nushell 补全支持
- 新增 clap_complete_nushell 依赖
- 支持 6 种 shell 补全：bash, zsh, fish, powershell, elvish, nushell
- 更新补全功能使用说明文档

### Changed

- 改进 shell 补全功能，支持更多 shell 类型
- 优化 CLI 模块的补全生成逻辑
- 更新 clap_complete 到最新版本 4.5.58

### Fixed

- 修复 nushell 补全生成问题
- 改进错误提示信息

## [0.8.0] - 2025-10-05

### Major Release

- 模块化重构 - 将单一 main.rs 拆分为多个功能模块
- 提高代码可维护性和可扩展性
- 保持所有原有功能不变
- 代码结构更清晰，便于后续开发

### Added

- 创建 cli 模块处理命令行参数和补全生成
- 创建 models 模块定义数据结构和常量
- 创建 package 模块处理包管理和版本查询
- 创建 updater 模块处理包更新功能
- 创建 display 模块处理用户界面和结果显示
- 重构 main.rs 为协调器，代码从 748 行减少到 180 行

### Changed

- 模块化架构，每个模块职责单一
- 提高代码可维护性和可扩展性
- 便于团队协作和后续功能开发

## [0.7.0] - 2025-10-03

### Major Release

- 升级到主要版本 0.7.0
- 标志着项目进入新的发展阶段
- 包含所有 0.6.x 系列的优化和改进
- 为未来的重大功能更新做准备

### Summary of 0.6.x Series

- 完整的代码重构和优化
- 改进的 GitHub Actions 工作流
- 极简化的发布流程
- 增强的代码可读性和维护性
- 统一的错误处理和进度显示

## [0.6.10] - 2025-10-03

### Improved

- 优化代码结构和可读性
- 提取公共函数，减少代码重复
- 改进导入语句组织
- 统一进度条创建逻辑
- 简化版本信息格式化
- 提高代码维护性

### Added

- 新增 `parse_package_line()` 函数用于解析包信息
- 新增 `create_progress_bar()` 和 `create_main_progress_bar()` 函数
- 新增 `format_version_info()` 函数统一版本信息显示
- 新增常量 `PROGRESS_TICK_MS` 和 `PROGRESS_BAR_WIDTH`

## [0.6.9] - 2025-10-03

### Fixed

- 修复 GitHub Action 中重复的步骤名称
- 将认证步骤重命名为 "Authenticate with Crates.io"
- 将发布步骤保持为 "Publish to Crates.io"
- 消除步骤名称混淆，提高工作流可读性

## [0.6.8] - 2025-10-03

### Changed

- 极简化 release.yml 发布流程
- 移除所有缓存步骤
- 移除测试检查
- 只保留核心发布功能
- 最大化发布速度
- 专注于 crates.io 发布

## [0.6.7] - 2025-10-03

### Changed

- 简化 release.yml 发布流程
- 移除复杂的 changelog 生成步骤
- 移除格式检查和 clippy 检查
- 保留基本的测试和缓存
- 专注于 crates.io 发布功能
- 添加成功发布的消息提示
- 简化工作流程，提高执行速度

## [0.6.6] - 2025-10-03

### Changed

- 分离 GitHub Action 工作流
- 将 release.yml 改为专门负责 crates.io 发布
- 创建 github-release.yml 专门负责 GitHub Release 创建
- 分离关注点，提高工作流的可维护性
- release.yml: 负责代码质量检查 + crates.io 发布
- github-release.yml: 负责 GitHub Release 创建

## [0.6.5] - 2025-10-03

### Fixed

- 修复 GitHub Release 403 权限问题
- 添加 contents: write 权限用于创建 releases
- 添加 pull-requests: write 权限用于 release 操作
- 在 Create Release 步骤中明确指定 GITHUB_TOKEN
- 解决 GitHub Action 创建 Release 时的权限不足问题

## [0.6.4] - 2025-10-03

### Changed

- 调整发布顺序，先发布 GitHub Release 再发布到 crates.io
- 先创建 GitHub Release（使用 release_notes.md）
- 然后清理工作目录（删除 release_notes.md）
- 最后发布到 crates.io（工作目录干净）
- 确保 GitHub Release 能够使用 release notes 文件
- 同时保持 crates.io 发布时工作目录干净

## [0.6.3] - 2025-10-03

### Fixed

- 调整清理步骤的顺序
- 将清理工作目录的步骤移到发布之后
- 确保 release_notes.md 在创建 GitHub Release 时可用
- 保持工作目录干净的同时不影响发布流程

## [0.6.2] - 2025-10-03

### Fixed

- 移除 --allow-dirty 标志，保持工作目录干净
- 添加 release_notes.md 到 .gitignore
- 在发布前添加清理工作目录的步骤
- 确保所有文件都是最新的，保持代码库的整洁性

## [0.6.1] - 2025-10-03

### Fixed

- 修复 GitHub Action 中 cargo publish 检测到未提交文件的问题
- 添加 --allow-dirty 标志确保发布流程能够正常完成
- 解决 release_notes.md 文件导致的发布失败问题

## [0.6.0] - 2025-10-03

### Added

- 添加自动 Release 和 Crates.io 发布功能
- 创建 GitHub Action 自动生成 Release
- 添加智能 changelog 生成，支持 emoji 格式化
- 集成 Crates.io 自动发布功能
- 添加代码质量检查 (测试、格式化、clippy)
- 支持缓存优化构建速度

### Changed

- 优化发布流程，实现完全自动化
- 每次推送标签时自动创建 Release 并发布到 crates.io

## [0.5.0] - 2025-10-03

### Added

- 添加更新摘要功能，显示详细的版本变化信息
- 新增 UpdateResult 结构体跟踪更新结果
- 添加 print_update_summary 函数显示更新摘要
- 在更新完成后显示每个包的具体版本变化
- 区分成功和失败的更新，提供清晰的版本对比

### Changed

- 修改 update_package 函数返回详细的更新结果信息
- 优化用户体验，提供更详细的更新反馈

## [0.4.2] - 2025-10-03

### Refactored

- 移除 install_completion.sh 脚本，简化项目结构
- 更新 README.md 提供简化的手动安装说明
- 优化代码结构，提升可维护性
- 减少项目复杂度，专注核心功能

## [0.4.1] - 2025-10-03

### Fixed

- 修复 cargo install 编译输出显示问题
- 只有在命令失败时才显示 stderr 作为错误信息
- 成功时不再显示正常的编译输出
- 避免将正常的编译过程误报为错误

## [0.4.0] - 2025-10-03

### Added

- 添加 Shell 补全支持 (zsh, bash, fish, powershell, elvish)
- 添加 `--completion` 参数生成补全脚本
- 支持多种 shell 的补全功能
- 添加详细的补全安装说明

### Changed

- 版本升级到 0.4.0
- 优化补全安装体验

## [0.3.0] - 2025-10-03

### Added

- 添加 GitHub Actions 工作流用于 CI/CD
- 添加自动发布到 crates.io 的功能
- 添加发布检查清单文档

### Changed

- 项目重命名为 `pkg-checker`
- 程序名称从 `cargo-update-checker` 改为 `pkg-checker`
- 避免与 cargo 自带命令混淆

### Fixed

- 修复 cargo install 命令参数顺序问题
- 修复预发布版本安装问题

## [0.2.0] - 2025-10-03

### Added

- 添加进度条显示，改善用户体验
- 添加交互式更新模式
- 添加智能预发布版本检测
- 添加彩色输出和友好的用户界面

### Changed

- 默认启用交互模式
- 优化版本比较逻辑
- 改进错误处理和重试机制

## [0.1.0] - 2025-10-03

### Added

- 初始版本发布
- 支持检查全局安装的 Cargo 包更新
- 支持稳定版本和预发布版本检测
- 支持批量更新包
- 支持命令行参数配置

[Unreleased]: https://github.com/jenkinpan/cargo-fresh/compare/v0.10.3...HEAD
[0.10.3]: https://github.com/jenkinpan/cargo-fresh/compare/v0.10.2...v0.10.3
[0.10.2]: https://github.com/jenkinpan/cargo-fresh/compare/v0.10.1...v0.10.2
[0.10.1]: https://github.com/jenkinpan/cargo-fresh/compare/v0.10.0...v0.10.1
[0.10.0]: https://github.com/jenkinpan/cargo-fresh/compare/v0.9.14...v0.10.0
[0.9.14]: https://github.com/jenkinpan/cargo-fresh/compare/v0.9.13...v0.9.14
[0.9.13]: https://github.com/jenkinpan/cargo-fresh/compare/v0.9.12...v0.9.13
[0.9.12]: https://github.com/jenkinpan/cargo-fresh/compare/v0.9.11...v0.9.12
[0.9.11]: https://github.com/jenkinpan/cargo-fresh/compare/v0.9.10...v0.9.11
[0.9.10]: https://github.com/jenkinpan/cargo-fresh/compare/v0.9.9...v0.9.10
[0.9.9]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.9.0...v0.9.9
[0.9.0]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.8.1...v0.9.0
[0.8.1]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.6.10...v0.7.0
[0.6.10]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.6.9...v0.6.10
[0.6.9]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.6.8...v0.6.9
[0.6.8]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.6.7...v0.6.8
[0.6.7]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.6.6...v0.6.7
[0.6.6]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.6.5...v0.6.6
[0.6.5]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.6.4...v0.6.5
[0.6.4]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.6.3...v0.6.4
[0.6.3]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.6.2...v0.6.3
[0.6.2]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.4.2...v0.5.0
[0.4.2]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jenkinpan/pkg-checker-rs/releases/tag/v0.1.0
