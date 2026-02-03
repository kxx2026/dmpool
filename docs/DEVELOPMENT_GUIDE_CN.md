# DMPool 开发文档

> 作者：Claude (Anthropic)
> 创建日期：2025-02-03
> 项目：DMPool - 开源比特币 PPLNS 矿池

---

## 目录

1. [项目概述](#项目概述)
2. [架构设计](#架构设计)
3. [快速开始](#快速开始)
4. [部署指南](#部署指南)
5. [API 文档](#api-文档)
6. [配置说明](#配置说明)
7. [故障排查](#故障排查)
8. [开发指南](#开发指南)

---

## 项目概述

DMPool 是基于 Hydra-Pool (原 256Foundation/Hydra-Pool) 改进的开源比特币 PPLNS 矿池。

### 核心特性

- ✅ **PPLNS 算法** - Pay Per Last N Shares，公平的收益分配
- ✅ **直接从 Coinbase 支付** - 矿池不托管资金
- ✅ **完整的管理后台** - Web 界面监控所有服务状态
- ✅ **企业级安全** - 配置验证、审计日志、权限控制
- ✅ **多网络支持** - Mainnet、Testnet4、Signet

### 技术栈

| 组件 | 技术 |
|------|------|
| 核心语言 | Rust (Edition 2024) |
| 挖矿协议 | Stratum V2 |
| 存储 | RocksDB (17 个 Column Families) |
| Web 框架 | Axum 0.7 |
| 前端 | TailwindCSS + Chart.js |
| 监控 | Prometheus + Grafana |

---

## 架构设计

### 系统架构图

```
┌─────────────────────────────────────────────────────────────────┐
│                       DMPool 系统架构                           │
└─────────────────────────────────────────────────────────────────┘

┌────────────┐    ┌──────────────┐    ┌─────────────┐
│  矿工客户端  │───►│  Stratum     │───►│  PPLNS      │
│ (BitAxe等) │    │  Server      │    │  核心计算   │
└────────────┘    └──────────────┘    └─────────────┘
                          │                   │
                          ▼                   ▼
                   ┌─────────────────────────────┐
                   │      RocksDB 存储层          │
                   │   (17 Column Families)      │
                   └─────────────────────────────┘
                          │
          ┌───────────┼───────────┐
          ▼           ▼           ▼
    ┌─────────┐ ┌──────────┐ ┌──────────┐
    │Bitcoin  │ │  Admin   │ │  API     │
    │ RPC/ZMQ │ │  Panel   │ │  Server  │
    └─────────┘ └──────────┘ └──────────┘
```

### 目录结构

```
dmpool/
├── src/
│   ├── bin/                      # 可执行文件
│   │   ├── dmpool.rs            # 主矿池服务
│   │   ├── dmpool_admin.rs      # 管理后台
│   │   ├── dmpool_health.rs     # 健康检查服务
│   │   └── dmpool_cli.rs        # CLI 工具
│   ├── admin/                    # 管理模块 (开发中)
│   │   ├── mod.rs
│   │   ├── dashboard.rs
│   │   ├── config.rs
│   │   └── workers.rs
│   ├── health/                   # 健康检查模块
│   │   └── mod.rs
│   ├── audit/                    # 审计日志 (开发中)
│   ├── auth/                     # 认证系统 (开发中)
│   ├── backup/                   # 备份模块
│   ├── config/                   # 配置管理
│   ├── migration/                # 数据迁移
│   └── reload/                   # 热重载
├── static/
│   └── admin/                    # 管理后台前端
│       └── index.html
├── config/
│   ├── homelab-signet.toml      # Signet 测试配置
│   └── config-example.toml      # 配置模板
├── scripts/
│   └── deploy-to-homelab.sh     # 部署脚本
├── Cargo.toml
└── README.md
```

---

## 快速开始

### 本地开发

```bash
# 1. 克隆仓库
git clone https://github.com/kxx2026/dmpool.git
cd dmpool

# 2. 安装依赖
cargo install --git https://github.com/p2poolv2/p2poolv2

# 3. 构建项目
cargo +nightly build --release

# 4. 配置
cp docker/config-example.toml config.toml
# 编辑 config.toml 中的 Bitcoin RPC 设置

# 5. 运行
./target/release/dmpool --config config.toml

# 6. 启动管理后台
CONFIG_PATH=config.toml ADMIN_PORT=8080 ./target/release/dmpool_admin
# 访问 http://localhost:8080
```

### Docker 部署

```bash
# 使用 Docker Compose (推荐)
docker compose -f docker-compose.yml up -d
```

---

## 部署指南

### 系统要求

- **操作系统**: Linux (Ubuntu 22.04+ 推荐)
- **内存**: 至少 2GB RAM
- **磁盘**: 至少 50GB SSD
- **网络**: 稳定的互联网连接
- **Bitcoin 节点**: 本地全节点或公共 RPC

### 部署步骤

#### 1. 使用自动部署脚本

```bash
chmod +x scripts/deploy-to-homelab.sh
./scripts/deploy-to-homelab.sh
```

#### 2. 手动部署

```bash
# 1. 创建用户和目录
sudo useradd -m -s /bin/bash dmpool
sudo su - dmpool
mkdir -p /home/dmpool/{data,logs,config}

# 2. 下载二进制文件
wget https://github.com/kxx2026/dmpool/releases/latest/download/dmpool
wget https://github.com/kxx2026/dmpool/releases/latest/download/dmpool_admin
chmod +x dmpool dmpool_admin

# 3. 配置文件
cp config-example.toml config/config.toml
# 编辑配置...

# 4. 创建 systemd 服务
sudo nano /etc/systemd/system/dmpool.service
sudo nano /etc/systemd/system/dmpool-admin.service

# 5. 启动服务
sudo systemctl daemon-reload
sudo systemctl enable dmpool dmpool-admin
sudo systemctl start dmpool dmpool-admin
```

### systemd 服务文件

**dmpool.service**
```ini
[Unit]
Description=DMPool Mining Pool
After=network.target bitcoin.service

[Service]
Type=simple
User=dmpool
WorkingDirectory=/home/dmpool
ExecStart=/home/dmpool/dmpool --config /home/dmpool/config/config.toml
Restart=always
RestartSec=10
StandardOutput=append:/home/dmpool/logs/dmpool.log
StandardError=append:/home/dmpool/logs/dmpool-error.log

[Install]
WantedBy=multi-user.target
```

**dmpool-admin.service**
```ini
[Unit]
Description=DMPool Admin Panel
After=network.target dmpool.service

[Service]
Type=simple
User=dmpool
WorkingDirectory=/home/dmpool
Environment="CONFIG_PATH=/home/dmpool/config/config.toml"
Environment="ADMIN_PORT=8080"
ExecStart=/home/dmpool/dmpool_admin
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

---

## API 文档

### 管理后台 API

| 端点 | 方法 | 描述 |
|------|------|------|
| `/` | GET | 管理后台首页 |
| `/api/health` | GET | 健康检查 |
| `/api/services/status` | GET | **关键服务状态** |
| `/api/dashboard` | GET | 仪表板数据 |
| `/api/config` | GET | 查看配置 |
| `/api/config` | POST | 更新配置 |
| `/api/config/reload` | POST | 重载配置 |
| `/api/workers` | GET | 矿工列表 |
| `/api/workers/:address` | GET | 矿工详情 |
| `/api/workers/:address/ban` | POST | 封禁矿工 |
| `/api/workers/:address/unban` | POST | 解封矿工 |
| `/api/safety/check` | GET | 安全检查 |

### /api/services/status 响应示例

```json
{
  "status": "ok",
  "data": {
    "status": "healthy",
    "database": {
      "status": "healthy",
      "message": "Database operational",
      "latency_ms": 2
    },
    "bitcoin_node": {
      "status": "healthy",
      "rpc_latency_ms": 45,
      "blockchain": {
        "blocks": 250000,
        "headers": 250000,
        "initial_block_download": false,
        "verification_progress": 0.9999,
        "best_block_hash": "0000..."
      },
      "network": {
        "connections": 12,
        "network_active": true
      },
      "sync_progress": 0.9999,
      "message": "已同步，高度: 250000，连接: 12 个节点"
    },
    "stratum": {
      "status": "healthy",
      "listening": true,
      "active_connections": 15,
      "shares_per_second": 125.5,
      "current_difficulty": 45.2,
      "message": "端口 3333 监听中，15 个活跃连接"
    },
    "zmq": {
      "status": "healthy",
      "message": "ZMQ 连接正常"
    },
    "uptime_seconds": 3600,
    "memory_mb": 256
  }
}
```

---

## 配置说明

### 关键配置项

| 配置项 | 默认值 | 说明 | 安全性 |
|--------|--------|------|--------|
| `pplns_ttl_days` | 7 | PPLNS 窗口大小（天） | ❌ **不可低于 7**，否则矿工损失收益 |
| `donation` | 无 | 开发者捐赠（基点） | ❌ **10000 = 100% 捐赠，矿工收益为0** |
| `ignore_difficulty` | false | 忽略难度验证 | ❌ **必须为 false** |
| `start_difficulty` | 32 | 起始难度 | 建议 32-512 |
| `minimum_difficulty` | 16 | 最低难度 | 建议 16-256 |

### 配置验证

系统会自动验证配置并给出警告：

```json
{
  "safe": false,
  "critical_issues": [
    {
      "severity": "critical",
      "param": "pplns_ttl_days",
      "message": "TTL=1天过短，标准为7天，矿工可能损失约85%的收益",
      "recommendation": "设置为 7"
    }
  ],
  "warnings": []
}
```

---

## 故障排查

### 常见问题

#### 1. 服务无法启动

**症状**: `Exec format error`

**原因**: 二进制文件架构不匹配

**解决方案**:
```bash
# 检查二进制格式
file dmpool

# 应该显示: ELF 64-bit LSB pie executable
# 如果显示 Mach-O，说明是在 macOS 上编译的
# 需要在目标机器上重新编译
```

#### 2. Bitcoin 节点连接失败

**症状**: `bitcoin_node.status = "unhealthy"`

**检查步骤**:
```bash
# 测试 RPC 连接
curl -X POST --user user:pass http://127.0.0.1:8332 \
  -H 'Content-Type: application/json;' \
  --data '{"jsonrpc":"2.0","id":"1","method":"getblockchaininfo","params":[]}'

# 检查 ZMQ
nc -zv 127.0.0.1 28334
```

#### 3. RocksDB 错误

**症状**: `IO error: No such file or directory`

**解决方案**:
```bash
# 检查数据目录权限
ls -la /home/dmpool/data/

# 首次运行需要初始化
# dmpool 会自动创建 RocksDB
```

### 日志查看

```bash
# 系统服务日志
sudo journalctl -u dmpool -f
sudo journalctl -u dmpool-admin -f

# 应用日志
tail -f /home/dmpool/logs/dmpool.log
```

---

## 开发指南

### 环境设置

```bash
# 安装 Rust (需要 nightly)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain nightly
source "$HOME/.cargo/env"

# 克隆项目
git clone https://github.com/kxx2026/dmpool.git
cd dmpool

# 构建项目
cargo +nightly build --bins
```

### 代码规范

- 使用 **Rust Edition 2024**
- 遵循 **Apple 质量标准**
- 所有公开 API 必须有文档注释
- 使用 `Result` 类型进行错误处理
- 避免 `unwrap()`，使用 `?` 传播错误

### 测试

```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test health::tests

# 带输出的测试
cargo test -- --nocapture
```

### 贡献指南

1. Fork 项目
2. 创建功能分支: `git checkout -b feature/amazing-feature`
3. 提交更改: `git commit -m "Add amazing feature"`
4. 推送分支: `git push origin feature/amazing-feature`
5. 创建 Pull Request

---

## 联系方式

- **GitHub**: https://github.com/kxx2026/dmpool
- **Issues**: https://github.com/kxx2026/dmpool/issues

---

## 许可证

AGPLv3 - 详见 [LICENSE](LICENSE) 文件
