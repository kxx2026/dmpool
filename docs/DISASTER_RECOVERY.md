# DMPool 故障恢复预案 (Disaster Recovery Plan)

本文档提供了 DMPool Bitcoin 矿池的故障诊断、恢复步骤和预防措施。

## 目录

- [故障诊断流程](#故障诊断流程)
- [常见故障场景](#常见故障场景)
- [数据恢复](#数据恢复)
- [预防措施](#预防措施)
- [紧急联系](#紧急联系)

## 故障诊断流程

### 第一步: 健康检查

使用健康检查端点快速诊断问题：

```bash
# 检查服务状态
curl http://localhost:8080/api/health

# 检查各组件状态
curl http://localhost:8080/api/services/status
```

### 第二步: 查看日志

```bash
# 查看主服务日志
journalctl -u dmpool -f

# 查看管理后台日志
journalctl -u dmpool-admin -f

# 查看 stratum 日志
tail -f /var/log/dmpool/stratum.log
```

### 第三步: 检查数据库

```bash
# 使用 dmpool_cli 检查数据库状态
./dmpool_cli db-stats

# 检查 RocksDB 文件完整性
./dmpool_cli db-check
```

## 常见故障场景

### 场景 1: 算力突然下降

**症状:**
- 仪表板显示的算力大幅下降
- 矿工报告的算力与实际不符

**诊断步骤:**

1. 检查网络连接
```bash
ping pool.example.com
telnet pool.example.com 3333
```

2. 检查 stratum 服务状态
```bash
systemctl status dmpool
journalctl -u dmpool -n 100
```

3. 检查难度设置
```bash
./dmpool_cli config-get | grep difficulty
```

**解决方案:**

- 如果网络问题：检查防火墙/路由器配置
- 如果 stratum 崩溃：重启服务 `systemctl restart dmpool`
- 如果难度过高：降低 `start_difficulty` 和 `minimum_difficulty`

**预防措施:**
- 设置合理的难度值（start: 32, minimum: 16）
- 配置告警监控算力下降

---

### 场景 2: 矿工无法连接

**症状:**
- 矿工显示 "Connection refused" 或 "Connection timeout"
- 新矿工无法加入

**诊断步骤:**

1. 检查端口监听
```bash
netstat -tuln | grep 3333
ss -tuln | grep 3333
```

2. 检查防火墙
```bash
iptables -L -n
ufw status
```

3. 检查配置
```bash
./dmpool_cli config-get
```

**解决方案:**

- 确保端口开放：`ufw allow 3333` 或 `iptables -A INPUT -p tcp --dport 3333 -j ACCEPT`
- 检查 `stratum.port` 配置是否正确
- 重启 stratum 服务

---

### 场景 3: 数据库损坏

**症状:**
- 服务启动失败
- 日志显示 RocksDB 错误
- 份额数据丢失

**诊断步骤:**

1. 检查数据库文件
```bash
ls -lah /path/to/data/
```

2. 尝试手动打开
```bash
./dmpool_cli db-check
```

**解决方案:**

**重要: 立即创建当前数据的备份！**

```bash
# 备份当前数据（即使损坏的）
cp -r /path/to/data /path/to/data_backup_$(date +%Y%m%d_%H%M%S)
```

然后恢复最近的备份：

```bash
# 通过 API 恢复
curl -X POST http://localhost:8080/api/backup/{backup_id}/restore \
  -H "Authorization: Bearer YOUR_TOKEN"

# 或手动恢复
tar -xzf backups/dmpool_backup_YYYYMMDD_HHMMSS.tar.gz -C /path/to/
```

---

### 场景 4: 内存不足

**症状:**
- 系统负载高
- 进程被 OOM killer 杀死
- 服务频繁重启

**诊断步骤:**

```bash
# 检查内存使用
free -h

# 检查进程内存
ps aux --sort=-%mem | head

# 查看 OOM 日志
dmesg | grep -i oom
```

**解决方案:**

1. 增加系统交换空间
```bash
dd if=/dev/zero of=/swapfile bs=1G count=4
chmod 600 /swapfile
mkswap /swapfile
swapon /swapfile
```

2. 调整 RocksDB 缓存大小
```toml
[rocksdb]
max_open_files = 1000
cache_size = 2147483648  # 2GB
```

3. 清理旧份额数据
```bash
./dmpool_cli cleanup-shares --days=30
```

---

### 场景 5: 支付问题

**症状:**
- 矿工未收到预期支付
- PPLNS 计算异常

**诊断步骤:**

1. 验证 PPLNS 设置
```bash
./dmpool_cli config-get | grep pplns
```

2. 使用验证工具
```bash
./dmpool_cli validate-pplns --block-height=HEIGHT
```

3. 检查钱包连接
```bash
./dmpool_cli wallet-status
```

**解决方案:**

- 确保在找到区块时正确记录了矿工份额
- 检查 `pplns_ttl_days` 设置（推荐 7 天）
- 验证比特币节点 RPC 连接

---

### 场景 6: 配置错误导致服务无法启动

**症状:**
- 服务启动后立即退出
- 配置重载失败

**诊断步骤:**

1. 验证配置文件
```bash
./dmpool --validate-config config.toml
```

2. 检查配置差异
```bash
./dmpool_cli config-diff config.toml
```

**解决方案:**

使用配置确认系统：

```bash
# 通过管理后台修改配置
curl -X POST http://localhost:8080/api/config \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"pplns_ttl_days": 7}'

# 确认更改
curl -X POST http://localhost:8080/api/config/confirmations/{id} \
  -H "Authorization: Bearer YOUR_TOKEN"

# 应用更改
curl -X POST http://localhost:8080/api/config/confirmations/{id}/apply \
  -H "Authorization: Bearer YOUR_TOKEN"
```

---

## 数据恢复

### 完整恢复流程

1. **停止所有服务**
```bash
systemctl stop dmpool
systemctl stop dmpool-admin
```

2. **备份当前数据**（如果可能）
```bash
cp -r /path/to/data /path/to/data_before_restore
```

3. **选择并解压备份**
```bash
# 列出可用备份
curl http://localhost:8080/api/backup/list \
  -H "Authorization: Bearer YOUR_TOKEN"

# 下载备份文件
scp user@backup-server:/backups/dmpool_backup_*.tar.gz ./

# 解压到临时位置
tar -xzf dmpool_backup_YYYYMMDD_HHMMSS.tar.gz -C /tmp/restore
```

4. **验证备份完整性**
```bash
# 检查 SHA256 校验和
sha256sum /tmp/restore/data/CURRENT

# 使用 dmpool_cli 验证
./dmpool_cli db-check --path=/tmp/restore/data
```

5. **替换数据目录**
```bash
# 删除损坏的数据
rm -rf /path/to/data/*

# 复制备份的数据
cp -r /tmp/restore/data/* /path/to/data/
```

6. **重启服务**
```bash
systemctl start dmpool
systemctl start dmpool-admin
```

7. **验证服务状态**
```bash
curl http://localhost:8080/api/health
curl http://localhost:8080/api/dashboard
```

### 紧急恢复: 最小化配置

如果主节点完全损坏，使用最小化配置快速恢复：

```toml
[network]
network = "signet"  # 或 "mainnet"

[stratum]
listen_address = "0.0.0.0"
port = 3333
start_difficulty = 32
minimum_difficulty = 16

[p2p]
bootstrap_peers = []

[store]
path = "./data"

[api]
enable = true
port = 8080
```

## 预防措施

### 1. 定期备份

**自动化备份脚本:**

```bash
#!/bin/bash
# /usr/local/bin/dmpool-backup.sh

BACKUP_DIR="/var/backups/dmpool"
DATA_DIR="/var/lib/dmpool/data"
RETENTION_DAYS=30

# 创建备份目录
mkdir -p "$BACKUP_DIR"

# 创建备份
BACKUP_NAME="dmpool_backup_$(date +%Y%m%d_%H%M%S).tar.gz"
tar -czf "$BACKUP_DIR/$BACKUP_NAME" -C "$(dirname $DATA_DIR)" "$(basename $DATA_DIR)"

# 通过 API 创建备份记录
curl -X POST http://localhost:8080/api/backup/create \
  -H "Authorization: Bearer $JWT_TOKEN"

# 清理旧备份
find "$BACKUP_DIR" -name "dmpool_backup_*.tar.gz" -mtime +$RETENTION_DAYS -delete

echo "Backup completed: $BACKUP_NAME"
```

**添加到 crontab:**
```bash
# 每天凌晨 2 点执行备份
0 2 * * * /usr/local/bin/dmpool-backup.sh
```

### 2. 监控告警

配置告警规则以提前发现问题：

```bash
# 添加算力下降告警
curl -X POST http://localhost:8080/api/alerts/rules \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Hashrate Drop Alert",
    "condition": {
      "type": "hashrate_below",
      "threshold": 100,
      "duration_minutes": 30
    },
    "level": "warning",
    "channels": ["telegram", "webhook"]
  }'

# 添加长时间未出块告警
curl -X POST http://localhost:8080/api/alerts/rules \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "No Block Alert",
    "condition": {
      "type": "no_block",
      "duration_minutes": 1440
    },
    "level": "critical",
    "channels": ["telegram"]
  }'
```

### 3. 配置管理最佳实践

- **永远不要在生产环境直接修改配置文件**
- **使用管理后台的配置确认系统**
- **修改重要参数前创建备份**
- **记录所有配置变更**（审计日志自动记录）

危险配置变更检查清单：

- [ ] 备份已创建
- [ ] 理解变更影响
- [ ] 测试环境已验证
- [ ] 回滚计划已准备
- [ ] 团队已通知

### 4. 安全措施

- 使用强 JWT secret（至少 32 字符）
- 启用 HTTPS（使用 nginx 反向代理）
- 限制管理后台访问 IP
- 定期审查审计日志
- 及时更新依赖包

### 5. 容量规划

| 组件 | 最小配置 | 推荐配置 | 大型矿池 |
|------|----------|----------|----------|
| CPU | 2 核 | 4 核 | 8+ 核 |
| 内存 | 4GB | 8GB | 16GB+ |
| 磁盘 | 50GB SSD | 200GB SSD | 500GB+ NVMe |
| 网络 | 100Mbps | 1Gbps | 10Gbps |

## 紧急联系

### 内部联系人

| 角色 | 姓名 | 联系方式 | 职责范围 |
|------|------|----------|----------|
| 运维负责人 | - | - | 基础设施、服务器 |
| 开发负责人 | - | - | 代码、配置 |
| 安全负责人 | - | - | 安全事件 |

### 外部资源

- **GitHub Issues**: https://github.com/kxx2026/dmpool/issues
- **P2Poolv2 文档**: https://github.com/p2poolv2/p2poolv2
- **Bitcoin 开发者社区**: https://bitcoin.stackexchange.com

## 附录

### A. 常用命令参考

```bash
# 服务管理
systemctl start dmpool
systemctl stop dmpool
systemctl restart dmpool
systemctl status dmpool

# 日志查看
journalctl -u dmpool -f           # 实时日志
journalctl -u dmpool -n 1000      # 最近 1000 行
journalctl -u dmpool --since today # 今天的日志

# 数据库操作
./dmpool_cli db-stats              # 数据库统计
./dmpool_cli db-check              # 数据库检查
./dmpool_cli cleanup-shares --days=30  # 清理旧份额

# 配置管理
./dmpool_cli config-get            # 查看配置
./dmpool_cli config-diff           # 配置对比

# 备份恢复
./dmpool_cli backup create         # 创建备份
./dmpool_cli backup list           # 列出备份
./dmpool_cli backup restore ID     # 恢复备份
```

### B. 配置安全检查清单

部署前必须检查的配置项：

```bash
#!/bin/bash
# 安全检查脚本

echo "Checking DMPool configuration..."

# 检查 JWT secret
if grep -q "CHANGE_THIS_SECRET" /etc/dmpool/config.toml; then
    echo "❌ ERROR: Default JWT secret in use!"
    exit 1
fi

# 检查端口绑定
if grep -q "listen_address = \"0.0.0.0\"" /etc/dmpool/config.toml; then
    echo "⚠️  WARNING: Stratum listening on all interfaces"
fi

# 检查 PPLNS TTL
TTL=$(grep "pplns_ttl_days" /etc/dmpool/config.toml | awk '{print $3}')
if [ "$TTL" -lt 7 ]; then
    echo "⚠️  WARNING: PPLNS TTL less than 7 days: $TTL"
fi

# 检查 donation
DONATION=$(grep "donation" /etc/dmpool/config.toml | awk '{print $3}')
if [ "$DONATION" -eq 10000 ]; then
    echo "❌ ERROR: Donation set to 100% (10000 basis points)!"
    exit 1
fi

echo "✅ Configuration check passed"
```

### C. 应急响应流程图

```
故障发生
    ↓
健康检查 (/api/health)
    ↓
┌─────────────┬─────────────┬─────────────┐
│             │             │             │
网络问题     服务崩溃    数据库错误  配置错误
│             │             │             │
检查网络     查看日志     备份恢复     配置回滚
重启服务     重启服务     恢复备份     重新加载
│             │             │             │
└─────────────┴─────────────┴─────────────┘
    ↓
验证服务状态
    ↓
记录事件 (审计日志)
    ↓
恢复完成
```

---

**最后更新:** 2025-01-XX  
**文档版本:** 1.0  
**维护者:** DMPool Team
