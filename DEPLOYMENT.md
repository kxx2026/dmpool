# Hydra-Pool 生产环境部署指南

> 版本: 2.4.0
> 更新时间: 2026-02-01
> 部署目标: 生产环境

---

## 目录

1. [系统要求](#1-系统要求)
2. [硬件配置标准](#2-硬件配置标准)
3. [网络架构设计](#3-网络架构设计)
4. [部署前准备](#4-部署前准备)
5. [安装部署](#5-安装部署)
6. [配置详解](#6-配置详解)
7. [监控告警](#7-监控告警)
8. [安全加固](#8-安全加固)
9. [运维管理](#9-运维管理)
10. [故障排查](#10-故障排查)

---

## 1. 系统要求

### 1.1 操作系统

| 环境 | 最低版本 | 推荐版本 | 状态 |
|------|----------|----------|------|
| Ubuntu | 22.04 LTS | 24.04 LTS | ✅ 完全支持 |
| Debian | 11 (Bullseye) | 12 (Bookworm) | ✅ 完全支持 |
| CentOS/RHEL | 8 | 9 Stream | ⚠️ 需额外依赖 |
| macOS | 13 (Ventura) | 14 (Sonoma) | ✅ 开发/测试 |

**生产环境推荐**: Ubuntu 24.04 LTS

### 1.2 软件依赖

#### 必需组件

- Rust 工具链: rustc 1.88.0+, cargo 1.88.0+
- 系统库: libssl-dev, pkg-config, clang 14+, libclang-dev
- 运行时库: libzmq5, libzstd1, libsnappy1v5, libbz2-1.0, liblz4-1

#### 安装命令

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install -y build-essential clang pkg-config libssl-dev libclang-dev libzmq3-dev cmake libzstd-dev libsnappy-dev libbz2-dev liblz4-dev zlib1g-dev

# Rust 安装
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

---

## 2. 硬件配置标准

### 2.1 最小配置（测试/开发环境）

> ⚠️ 不适用于生产环境

| 组件 | 规格 | 说明 |
|------|------|------|
| **CPU** | 2核心 | x86_64 或 ARM64 |
| **内存** | 2 GB | 4 GB 推荐 |
| **存储** | 20 GB SSD | RocksDB 性能依赖 SSD |
| **网络** | 100 Mbps | 上行带宽 |

### 2.2 标准配置（小型生产环境）

> ✅ 推荐：< 100 矿机并发

| 组件 | 规格 | 说明 |
|------|------|------|
| **CPU** | 4核心 @ 2.5GHz+ | 推荐 AMD Ryzen 5 或 Intel i5 |
| **内存** | 8 GB | 16 GB 更佳 |
| **存储** | 100 GB NVMe SSD | 系统盘 + 数据盘分离 |
| **网络** | 1 Gbps | 对称带宽，低延迟 |
| **公网IP** | 必需 | 固定 IP 或浮动 IP |

### 2.3 高性能配置（中型生产环境）

> ✅ 推荐：100-1000 矿机并发

| 组件 | 规格 | 说明 |
|------|------|------|
| **CPU** | 8核心 @ 3.0GHz+ | 推荐 AMD Ryzen 7/9 或 Intel i7 |
| **内存** | 16 GB | 32 GB 更佳 |
| **系统盘** | 200 GB NVMe SSD | 操作系统 + 应用 |
| **数据盘** | 500 GB NVMe SSD | RocksDB 专用 |
| **网络** | 10 Gbps | 数据中心级别 |

### 2.4 企业配置（大型生产环境）

> ✅ 推荐：> 1000 矿机并发

| 组件 | 规格 | 说明 |
|------|------|------|
| **CPU** | 16+ 核心 @ 3.5GHz+ | AMD EPYC 或 Intel Xeon |
| **内存** | 32+ GB | 64 GB 更佳 |
| **系统盘** | 500 GB NVMe SSD | RAID1 镜像 |
| **数据盘** | 2 TB NVMe SSD | RAID10 阵列 |
| **网络** | 10 Gbps+ | 冗余链路 |

---

## 3. 网络架构设计

### 3.1 端口规划

| 端口 | 服务 | 公网访问 | 认证 |
|------|------|----------|------|
| 3333 | Stratum 挖矿 | ✅ 必须 | 否 |
| 46884 | API 服务 | ⚠️ 可选 | Basic Auth |
| 3000 | Grafana 面板 | ⚠️ 可选 | 用户名/密码 |
| 9090 | Prometheus | ❌ 本地 | Basic Auth |

### 3.2 带宽需求

| 矿机数 | 带宽需求 | 推荐配置 |
|--------|----------|----------|
| < 50 | 50 Mbps | 家庭宽带 |
| 50-200 | 200 Mbps | 企业专线 |
| 200-500 | 500 Mbps | 数据中心级别 |
| 500+ | 1+ Gbps | 多线冗余 |

---

## 4. 部署前准备

### 4.1 比特币节点要求

```ini
# bitcoin.conf

# RPC 访问配置
rpcbind=0.0.0.0
rpcallowip=127.0.0.1
rpcallowip=<Pool服务器IP>
rpcallowip=172.16.0.0/12

# ZMQ 区块通知 (必需)
zmqpubhashblock=tcp://0.0.0.0:28334

# Coinbase 空间预留
blockmaxweight=3930000
```

---

## 5. 安装部署

### 5.1 方式一：Docker Compose（推荐）

```bash
cd /opt
mkdir dmpool
cd dmpool

curl --proto '=https' --tlsv1.2 -LsSf -o docker-compose.yml \
    https://github.com/256-Foundation/dmpool/releases/latest/download/docker-compose.yml

curl --proto '=https' --tlsv1.2 -LsSf -o config.toml \
    https://github.com/256-Foundation/dmpool/releases/latest/download/config-example.toml

nano config.toml
docker compose up -d
```

---

## 6. 配置详解

### 6.1 核心配置项

```toml
[store]
path = "/var/lib/dmpool/store.db"
background_task_frequency_hours = 24
pplns_ttl_days = 7

[stratum]
hostname = "0.0.0.0"
port = 3333
start_difficulty = 1
minimum_difficulty = 1
bootstrap_address = "bc1q YOUR_ADDRESS"
zmqpubhashblock = "tcp://<比特币节点IP>:28334"
network = "main"
```

---

## 7. 监控告警

### 7.1 Prometheus 监控

| 指标 | 说明 | 告警阈值 |
|------|------|----------|
| `shares_accepted_total` | 接受的 shares | 下降 >50% |
| `shares_rejected_total` | 拒绝的 shares | >5% 比率 |
| pool_hashrate | 矿池算力 | 下降 >30% |

---

## 8. 安全加固

### 8.1 防火墙配置

```bash
sudo ufw allow 22/tcp
sudo ufw allow 3333/tcp
sudo ufw allow 46884/tcp
sudo ufw enable
```

---

## 9. 运维管理

### 9.1 备份策略

```bash
#!/bin/bash
BACKUP_DIR="/backup/dmpool"
DATE=$(date +%Y%m%d_%H%M%S)
mkdir -p "$BACKUP_DIR"
docker run --rm -v dmpool_data:/data -v "$BACKUP_DIR":/backup alpine tar czf "/backup/dmpool_${DATE}.tar.gz" /data
```

---

## 10. 故障排查

### 10.1 常见问题

#### 问题 1: ZMQ 连接失败

**解决方案**:
1. 检查比特币节点 ZMQ 配置
2. 验证端口 28334 开放
3. 检查防火墙规则

---

*文档版本: 2.4.0*
*最后更新: 2026-02-01*
