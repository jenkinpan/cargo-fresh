# Changelog

所有重要的项目变更都会记录在这个文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
并且此项目遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased]

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

[Unreleased]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.9.0...HEAD
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
