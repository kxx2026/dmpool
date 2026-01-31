# 为 DMPool 贡献

感谢您对 DMPool 项目的关注和贡献！

> **注意**: DMPool 是 [256 Foundation](https://github.com/256-Foundation) 的 [Hydrapool](https://github.com/256-Foundation/Hydra-Pool) 项目的衍生版本。所有贡献必须符合 AGPLv3 许可证要求。

## 如何贡献

### 报告问题

在创建问题报告之前，请先检查现有 issues 以避免重复。

提交问题报告时，请包含：
- **DMPool 版本**: 运行 `dmpool --version`
- **Rust 版本**: 运行 `rustc --version`
- **操作系统**: 如 Ubuntu 24.04 LTS
- **重现步骤**: 详细的复现步骤
- **预期行为**: 您期望发生什么
- **实际行为**: 实际发生了什么
- **日志**: 相关日志输出（使用 ``` 代码块）

### 功能建议

欢迎提出改进建议！请：
- 使用清晰明确的标题
- 详细解释新功能
- 说明为什么这个改进有用
- 包含功能如何工作的示例

### 提交 Pull Request

1. **Fork 本仓库**
2. **创建分支**: `git checkout -b feature/your-feature-name`
3. **进行修改**
4. **编写测试**: 确保测试覆盖率
5. **更新文档**: 更新相关文档
6. **提交**: 使用清晰的提交信息
   ```
   feat: 添加新功能描述
   fix: 修复问题描述
   docs: 更新文档
   ```
7. **推送**: `git push origin feature/your-feature-name`
8. **创建 Pull Request**: 清楚地解释您的更改

## 开发环境设置

```bash
# 克隆您的 fork
git clone https://github.com/YOUR_USERNAME/dmpool.git
cd dmpool

# 安装 Rust 依赖
cargo build

# 运行测试
cargo test

# 调试模式运行
RUST_LOG=debug cargo run
```

## 代码风格

- 遵循 Rust 规范: `cargo fmt`
- 检查 clippy 警告: `cargo clippy -- -W warnings`
- 编写清晰、自文档化的代码
- 为复杂逻辑添加注释

## 许可证要求

**重要**: 所有贡献必须以 **AGPLv3** 许可证发布。

通过贡献，您同意您的贡献将：
1. 以 AGPLv3 许可证发布
2. 正确署名为作者
3. 符合原 Hydrapool 项目的许可证要求

您的版权声明将添加到 AUTHORS 文件中，包含：
- 您的姓名
- 您的邮箱（可选）
- GitHub 个人资料链接

## 我们需要帮助的领域

- **测试**: 目前测试覆盖率为 0%。帮助我们添加测试！
- **文档**: 改进现有文档或添加翻译
- **Bug 修复**: 查看 issues 标签页
- **功能**: 提出新功能或实现已请求的功能

## 获取帮助

- **讨论**: [GitHub Discussions](https://github.com/kxx2026/dmpool/discussions)
- **问题**: [GitHub Issues](https://github.com/kxx2026/dmpool/issues)

---

感谢您为 DMPool 做出贡献！🎉
