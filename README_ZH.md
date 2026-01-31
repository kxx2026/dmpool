# DMPool

<div align="center">

**DMPool** 是一个开源的比特币挖矿池软件，实现 PPLNS 收益分配，支持直接从 coinbase 支付。

[![Rust](https://img.shields.io/badge/rust-1.88.0+-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-AGPLv3-blue.svg)](./LICENSE)
[![GitHub](https://img.shields.io/badge/source-kxx2026%2Fdmpool-green.svg)](https://github.com/kxx2026/dmpool)

</div>

## 简介

DMPool 让您能够运行自己的比特币挖矿池，实现**零托管** — 所有收益直接从 coinbase 交易支付，矿池运营商不接触用户资金。

### 核心特性

| 特性 | 说明 |
|------|------|
| **非托管** | 收益直接从 coinbase 支付，无需信任 |
| **PPLNS 算法** | 基于贡献算力的公平分配 |
| **透明可验证** | 公开 API 支持份额和收益审计 |
| **监控面板** | 集成 Prometheus/Grafana 仪表盘 |
| **广泛兼容** | 支持任何 Bitcoin RPC 节点 |
| **易于扩展** | Rust 实现，支持自定义 |

## 快速开始

### Docker 部署（推荐）

```bash
# 下载配置文件
curl -fsSL https://github.com/kxx2026/dmpool/releases/latest/download/docker-compose.yml -o docker-compose.yml
curl -fsSL https://github.com/kxx2026/dmpool/releases/latest/download/config-example.toml -o config.toml

# 编辑配置文件
nano config.toml

# 启动矿池
docker compose up -d
```

服务地址：
- **Stratum**: `stratum://localhost:3333`
- **API**: `http://localhost:46884`
- **监控面板**: `http://localhost:3000`

### 二进制安装

```bash
curl -fsSL https://github.com/kxx2026/dmpool/releases/latest/download/dmpool-installer.sh | sh
```

## 配置说明

编辑 `config.toml`：

```toml
[store]
path = "./store.db"
pplns_ttl_days = 1

[stratum]
hostname = "0.0.0.0"
port = 3333
bootstrap_address = "bc1q...你的地址"
zmqpubhashblock = "tcp://127.0.0.1:28334"
network = "main"
pool_signature = "dmpool"

[bitcoinrpc]
url = "http://127.0.0.1:8332"
username = "bitcoin"
password = "你的RPC密码"

[api]
hostname = "0.0.0.0"
port = 46884
auth_user = "dmpool"
auth_token = "生成的令牌"
```

生成认证令牌：

```bash
dmpool_cli gen-auth <用户名> <密码>
```

## 源码编译

```bash
# 安装依赖 (Ubuntu/Debian)
sudo apt install -y libssl-dev pkg-config clang libclang-dev

# 克隆并编译
git clone https://github.com/kxx2026/dmpool.git
cd dmpool
cargo build --release

# 运行
./target/release/dmpool
```

**系统要求**: Rust 1.88.0+

## 架构设计

```
┌─────────────────────────────────────────────────────────────┐
│                         矿工                                │
│  (stratum://pool:3333)                                      │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                      DMPool 核心                            │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────────┐   │
│  │  Stratum    │  │   PPLNS      │  │   Coinbase      │   │
│  │  服务器     │─▶│   引擎       │─▶│   构建器        │   │
│  └─────────────┘  └──────────────┘  └─────────────────┘   │
│         │                    │                    │         │
└─────────┼────────────────────┼────────────────────┼─────────┘
          │                    │                    │
          ▼                    ▼                    ▼
    ┌─────────┐          ┌──────────┐         ┌──────────┐
    │  Rocks  │          │ Prometheus│        │ Bitcoin  │
    │    DB   │          │   API    │         │   RPC    │
    └─────────┘          └──────────┘         └──────────┘
```

## API 接口

| 端点 | 说明 |
|------|------|
| `GET /health` | 健康检查 |
| `GET /pplns_shares` | 下载所有 PPLNS 份额（JSON） |
| `GET /pplns_shares?start_time=X&end_time=Y` | 按时间过滤份额 |

## 监控面板

```bash
docker compose up -d prometheus grafana
```

仪表盘包含：
- 矿池算力和每秒份额数
- 用户和矿工统计
- 难度追踪
- 运行时间监控

## 比特币节点配置

在 `bitcoin.conf` 中调整 `blockmaxweight` 为 coinbase 输出预留空间：

```ini
# 为约 500 个 P2PKH 输出预留空间
blockmaxweight=3930000
```

| 输出数量 | Coinbase 权重 | 推荐 `blockmaxweight` |
|----------|---------------|------------------------|
| 100      | ~13,808 WU    | 3,986,000              |
| 500      | ~68,208 WU    | 3,930,000              |
| 1,000    | ~136,208 WU   | 3,860,000              |

## 安全建议

- **API 认证**: 配置 `auth_user` 和 `auth_token`
- **防火墙**: 限制 API 访问来源 IP
- **HTTPS**: 使用 nginx/Caddy 作为反向代理
- **及时更新**: 关注安全补丁

## 文档

- [部署指南](./DEPLOYMENT.md) — 生产环境部署
- [更新日志](./CHANGELOG.md) — 版本历史

## 许可证

本项目采用 **AGPLv3** 许可证 — 详见 [LICENSE](./LICENSE)

## 贡献

欢迎贡献！请：
1. Fork 本仓库
2. 创建特性分支
3. 提交更改
4. 发起 Pull Request

## 支持

- **问题反馈**: [GitHub Issues](https://github.com/kxx2026/dmpool/issues)
- **讨论交流**: [GitHub Discussions](https://github.com/kxx2026/dmpool/discussions)

---

**DMPool** — 去中心化比特币挖矿池
