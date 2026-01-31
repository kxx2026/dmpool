# DMPool 项目代码审查指南

> 本文档帮助新同事快速了解 DMPool 项目架构，进行代码审查和开发工作。

## 目录

1. [项目概述](#项目概述)
2. [技术栈](#技术栈)
3. [项目结构](#项目结构)
4. [核心模块详解](#核心模块详解)
5. [开发环境搭建](#开发环境搭建)
6. [代码审查清单](#代码审查清单)
7. [测试指南](#测试指南)
8. [常见问题](#常见问题)

---

## 项目概述

### 什么是 DMPool？

DMPool 是一个**去中心化比特币挖矿池**软件，基于 [Hydrapool](https://github.com/256-Foundation/Hydra-Pool) 衍生而来。

### 核心特性

- **非托管**: 收益直接从 coinbase 支付，矿池运营商不接触资金
- **PPLNS 算法**: 基于贡献算力的公平分配机制
- **透明可验证**: 公开 API 支持份额和收益审计
- **Rust 实现**: 高性能、内存安全

### 项目起源

| 项目 | 说明 |
|------|------|
| 原项目 | Hydrapool by 256 Foundation |
| 衍生项目 | DMPool by kxx |
| 许可证 | AGPLv3 |

---

## 技术栈

### 核心技术

| 技术 | 版本 | 用途 |
|------|------|------|
| Rust | 1.88.0+ | 主要开发语言 |
| Tokio | 1.0 | 异步运行时 |
| Bitcoin | 0.32.5 | 比特币相关 |
| RocksDB | 嵌入式 | 数据存储 |
| Axum | 0.7 | Web 框架 |

### 外部依赖

- p2poolv2_lib - 核心库
- p2poolv2_cli - CLI 工具
- p2poolv2_api - API 服务器
- bitcoindrpc - Bitcoin RPC 客户端

---

## 项目结构

```
dmpool/
├── src/
│   ├── main.rs                  # 主程序入口 (308 行)
│   ├── lib.rs                   # 库入口
│   ├── health/
│   │   └── mod.rs               # 健康检查模块
│   ├── config/
│   │   └── mod.rs               # 配置验证模块
│   └── bin/
│       ├── dmpool               # 矿池主程序
│       ├── dmpool_cli           # CLI 工具
│       └── dmpool_health        # 健康检查服务
├── tests/
│   ├── common/mod.rs            # 测试工具
│   └── basic_startup_test.rs   # 集成测试
├── config.toml                  # 配置文件
├── Cargo.toml                   # Rust 项目配置
└── README.md                     # 项目文档
```

---

## 核心模块详解

### 1. 主程序入口 (src/main.rs)

**职责**: 应用启动、组件协调、信号处理

**关键改进**:
- 配置加载失败返回错误而非 panic
- 数据库初始化失败返回错误
- SIGTERM 处理失败时优雅降级
- 关闭信号发送失败记录警告而非 panic

**代码审查要点**:
- 无 unwrap() 在生产路径
- 所有可恢复错误返回 Result
- 信号处理有容错

### 2. 健康检查模块 (src/health/mod.rs)

**职责**: 监控各组件健康状态

**API**:
``rust
pub struct HealthStatus {
    pub status: String,           // "healthy" | "degraded"
    pub database: ComponentStatus,
    pub bitcoin_rpc: ComponentStatus,
    pub zmq: ComponentStatus,
    pub uptime_seconds: u64,
}

pub struct HealthChecker {
    pub async fn check(&self) -> HealthStatus
}
```

**使用方式**:
``bash
CONFIG_PATH=config.toml HEALTH_PORT=8080 ./target/release/dmpool_health
curl http://localhost:8080/health
```

### 3. 配置验证模块 (src/config/mod.rs)

**职责**: 运行前配置验证

**验证规则**:
- 端口范围: 1024-65535
- 主机名格式验证
- 网络类型: main | signet | testnet4
- API 安全: 必须是 localhost
- Pool 签名: 最大 16 字节

---

## 开发环境搭建

### 系统要求

- OS: Ubuntu 24.04+ / macOS / Windows (WSL2)
- Rust: 1.88.0+
- 内存: 最少 4GB

### 安装步骤

```bash
# 1. 克隆项目
git clone https://github.com/kxx2026/dmpool.git
cd dmpool

# 2. 安装依赖 (Ubuntu/Debian)
sudo apt install -y libssl-dev pkg-config clang libclang-dev

# 3. 构建
cargo build --release

# 4. 运行测试
cargo test
```

---

## 代码审查清单

### 必查项

- [ ] **无 panic**: 代码中没有 unwrap()、expect() 在生产路径
- [ ] **错误传播**: 使用 Result<> 而非 panic
- [ ] **测试覆盖**: 新功能有对应测试
- [ ] **文档更新**: README/注释同步更新
- [ ] **许可证合规**: 保持 AGPLv3 兼容

### 推荐项

- [ ] **日志记录**: 关键操作有 info!/error!
- [ ] **性能考虑**: 避免 clone() 大对象
- [ ] **安全性**: 敏感操作有验证
- [ ] **代码风格**: cargo fmt 格式化
- [ ] **Clippy 通过**: cargo clippy 无警告

---

## 测试指南

### 运行测试

```bash
cargo test                    # 所有测试
cargo test --lib health      # 健康检查模块
cargo test --lib config      # 配置验证模块
cargo test --test integration  # 集成测试
```

### 当前测试状态

总计: 8 tests passing

---

## 代码审查流程

### 1. 创建 PR 前检查

```bash
cargo fmt
cargo clippy -- -W warnings
cargo test
cargo build --release
```

### 2. PR 提交模板

**必填**:
- 变更说明
- 变更类型
- 测试状态
- 审查要点

---

## 常见问题

### Q: 如何运行调试版本？

```bash
cargo build
RUST_LOG=debug ./target/debug/dmpool --config config.toml
```

### Q: 如何添加新的配置项？

1. 在 p2poolv2_lib/config.rs 中添加字段
2. 更新 config.toml.example
3. 在配置验证模块中添加验证规则
4. 添加测试

### Q: AGPLv3 合规性？

作为衍生项目必须:
1. 保留原作者署名
2. 开源所有修改
3. 向用户提供源代码 (网络服务)

---

**文档版本**: v1.0
**更新日期**: 2025-01-31
**维护者**: kxx
