# Changelog

所有重要的项目变更都会记录在这个文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
并且此项目遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased]

## [0.6.4] - 2024-12-19

### Changed

- 调整发布顺序，先发布 GitHub Release 再发布到 crates.io
- 先创建 GitHub Release（使用 release_notes.md）
- 然后清理工作目录（删除 release_notes.md）
- 最后发布到 crates.io（工作目录干净）
- 确保 GitHub Release 能够使用 release notes 文件
- 同时保持 crates.io 发布时工作目录干净

## [0.6.3] - 2024-12-19

### Fixed

- 调整清理步骤的顺序
- 将清理工作目录的步骤移到发布之后
- 确保 release_notes.md 在创建 GitHub Release 时可用
- 保持工作目录干净的同时不影响发布流程

## [0.6.2] - 2024-12-19

### Fixed

- 移除 --allow-dirty 标志，保持工作目录干净
- 添加 release_notes.md 到 .gitignore
- 在发布前添加清理工作目录的步骤
- 确保所有文件都是最新的，保持代码库的整洁性

## [0.6.1] - 2024-12-19

### Fixed

- 修复 GitHub Action 中 cargo publish 检测到未提交文件的问题
- 添加 --allow-dirty 标志确保发布流程能够正常完成
- 解决 release_notes.md 文件导致的发布失败问题

## [0.6.0] - 2024-12-19

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

## [0.5.0] - 2024-12-19

### Added

- 添加更新摘要功能，显示详细的版本变化信息
- 新增 UpdateResult 结构体跟踪更新结果
- 添加 print_update_summary 函数显示更新摘要
- 在更新完成后显示每个包的具体版本变化
- 区分成功和失败的更新，提供清晰的版本对比

### Changed

- 修改 update_package 函数返回详细的更新结果信息
- 优化用户体验，提供更详细的更新反馈

## [0.4.2] - 2024-12-19

### Refactored

- 移除 install_completion.sh 脚本，简化项目结构
- 更新 README.md 提供简化的手动安装说明
- 优化代码结构，提升可维护性
- 减少项目复杂度，专注核心功能

## [0.4.1] - 2024-12-19

### Fixed

- 修复 cargo install 编译输出显示问题
- 只有在命令失败时才显示 stderr 作为错误信息
- 成功时不再显示正常的编译输出
- 避免将正常的编译过程误报为错误

## [0.4.0] - 2024-12-19

### Added

- 添加 Shell 补全支持 (zsh, bash, fish, powershell, elvish)
- 添加 `--completion` 参数生成补全脚本
- 支持多种 shell 的补全功能
- 添加详细的补全安装说明

### Changed

- 版本升级到 0.4.0
- 优化补全安装体验

## [0.3.0] - 2024-12-19

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

## [0.2.0] - 2024-12-19

### Added

- 添加进度条显示，改善用户体验
- 添加交互式更新模式
- 添加智能预发布版本检测
- 添加彩色输出和友好的用户界面

### Changed

- 默认启用交互模式
- 优化版本比较逻辑
- 改进错误处理和重试机制

## [0.1.0] - 2024-12-19

### Added

- 初始版本发布
- 支持检查全局安装的 Cargo 包更新
- 支持稳定版本和预发布版本检测
- 支持批量更新包
- 支持命令行参数配置

[Unreleased]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/jenkinpan/pkg-checker-rs/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jenkinpan/pkg-checker-rs/releases/tag/v0.1.0
