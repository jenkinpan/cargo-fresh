# 发布检查清单

## 发布前检查

### 1. 代码质量检查

- [ ] 运行 `cargo clippy` 检查代码质量
- [ ] 运行 `cargo fmt` 格式化代码
- [ ] 运行 `cargo test` 确保所有测试通过
- [ ] 检查是否有未提交的更改

### 2. 版本管理

- [ ] 更新 `Cargo.toml` 中的版本号
- [ ] 更新 `CHANGELOG.md`（如果有）
- [ ] 提交版本更新

### 3. 文档检查

- [ ] 更新 `README.md` 中的版本信息
- [ ] 检查所有示例代码是否有效
- [ ] 确保文档链接正确

### 4. 发布准备

- [ ] 创建 Git 标签：`git tag v0.x.x`
- [ ] 推送标签：`git push origin v0.x.x`
- [ ] 确保 GitHub Actions 有 `CARGO_REGISTRY_TOKEN` 权限

## 发布流程

### 自动发布（推荐）

1. 创建并推送版本标签：

   ```bash
   git tag v0.3.0
   git push origin v0.3.0
   ```

2. GitHub Actions 会自动：
   - 运行测试和检查
   - 构建项目
   - 发布到 crates.io

### 手动发布

如果自动发布失败，可以手动发布：

```bash
cargo publish --token <your-token>
```

## 发布后检查

- [ ] 检查 crates.io 上的包信息
- [ ] 测试从 crates.io 安装：`cargo install pkg-checker`
- [ ] 更新项目文档中的安装说明
- [ ] 在 GitHub 上创建 Release

## 故障排除

### 常见问题

1. **Token 权限不足**：确保 `CARGO_REGISTRY_TOKEN` 有发布权限
2. **版本冲突**：检查 crates.io 上是否已存在相同版本
3. **依赖问题**：确保所有依赖都可用

### 重新发布

如果需要重新发布相同版本：

```bash
cargo yank --version 0.3.0 pkg-checker
cargo publish --token <your-token>
```
