# Contributing to DMPool

Thank you for your interest in contributing to DMPool!

> **Note**: DMPool is a fork of [Hydrapool](https://github.com/256-Foundation/Hydra-Pool) by 256 Foundation. All contributions must comply with the AGPLv3 license requirements.

## How to Contribute

### Reporting Bugs

Before creating bug reports, please check existing issues to avoid duplicates.

When filing a bug report, include:
- **DMPool version**: Run `dmpool --version`
- **Rust version**: Run `rustc --version`
- **Operating System**: e.g., Ubuntu 24.04 LTS
- **Steps to reproduce**: Detailed reproduction steps
- **Expected behavior**: What you expected to happen
- **Actual behavior**: What actually happened
- **Logs**: Relevant log output (use ``` code blocks)

### Suggesting Enhancements

Enhancement suggestions are welcome! Please:
- Use a clear and descriptive title
- Provide a detailed explanation of the feature
- Explain why this enhancement would be useful
- Include examples of how the feature would work

### Pull Requests

1. **Fork the repository**
2. **Create a branch**: `git checkout -b feature/your-feature-name`
3. **Make your changes**
4. **Write tests**: Ensure test coverage is maintained
5. **Update documentation**: Update relevant docs for your changes
6. **Commit**: Use clear commit messages
   ```
   feat: add new feature description
   fix: resolve bug description
   docs: update documentation
   ```
7. **Push**: `git push origin feature/your-feature-name`
8. **Create Pull Request**: Explain your changes clearly

## Development Setup

```bash
# Clone your fork
git clone https://github.com/YOUR_USERNAME/dmpool.git
cd dmpool

# Install Rust dependencies
cargo build

# Run tests
cargo test

# Run with debug output
RUST_LOG=debug cargo run
```

## Code Style

- Follow Rust conventions: `cargo fmt`
- Check for clippy warnings: `cargo clippy -- -W warnings`
- Write clear, self-documenting code
- Add comments for complex logic

## License Requirements

**IMPORTANT**: All contributions must be licensed under **AGPLv3**.

By contributing, you agree that your contributions will be:
1. Licensed under AGPLv3
2. Attributed properly to you as the author
3. Compliant with the original Hydrapool project's license

Your copyright notice will be added to the AUTHORS file along with:
- Your name
- Your email (optional)
- GitHub profile URL

## Areas Where We Need Help

- **Tests**: We currently have 0% test coverage. Help us add tests!
- **Documentation**: Improve existing docs or add translations
- **Bug fixes**: Check the issues tab
- **Features**: Propose new features or implement requested ones

## Getting Help

- **Discussions**: [GitHub Discussions](https://github.com/kxx2026/dmpool/discussions)
- **Issues**: [GitHub Issues](https://github.com/kxx2026/dmpool/issues)

---

Thank you for contributing to DMPool! 🎉
