# 安全政策

## 支持的版本

目前，仅为最新发布版本提供安全更新：

| 版本 | 支持状态 |
|------|----------|
| 2.4.x   | ✅        |
| < 2.4.0 | ❌        |

## 报告漏洞

**请勿**在公开 issues 中报告安全漏洞。

### 如何报告

发送邮件至：**security@dmpool.org**（或创建 [GitHub Security Advisory](https://github.com/kxx2026/dmpool/security/advisories)）

请包含：
- **描述**: 漏洞的清晰描述
- **影响**: 漏洞的潜在影响
- **重现步骤**: 重现步骤（如适用）
- **概念验证**: 演示问题的任何代码或截图

### 预期处理流程

1. **确认**: 我们会在 48 小时内确认收到
2. **评估**: 我们将评估严重程度并确定修复时间表
3. **协调**: 我们将与您协调公开披露
4. **披露**: 修复发布后，我们将披露漏洞信息

### 运营者安全最佳实践

在生产环境中部署 DMPool 时：

#### 1. API 认证

```toml
[api]
auth_user = "admin"
auth_token = "生成强令牌"
```

生成安全令牌：
```bash
dmpool_cli gen-auth admin "强密码_32位字符以上"
```

#### 2. 网络安全

```bash
# 防火墙 - 仅允许必要端口
sudo ufw allow 3333/tcp  # Stratum
sudo ufw allow 22/tcp    # SSH
sudo ufw enable

# API 应仅限内部访问
# 使用 nginx 反向代理提供外部访问，并配置限流
```

#### 3. Bitcoin RPC 安全

```ini
# bitcoin.conf
rpcuser=修改此项
rpcpassword=修改此项_使用强密码
rpcallowip=127.0.0.1
rpcbind=127.0.0.1
```

#### 4. 系统加固

```bash
# 以非 root 用户运行
sudo useradd -r -s /bin/false dmpool

# 配置 fail2ban
sudo apt install fail2ban

# 保持系统更新
sudo unattended-upgrades
```

#### 5. 定期审计

- 每周审查访问日志
- 监控异常算力模式
- 保持依赖更新
- 审查 Prometheus 告警

### AGPLv3 安全义务

根据 AGPLv3 要求：
- 如果您修改软件并将其作为网络服务运行，必须向用户提供源代码
- 您所做的任何安全修复必须回馈给社区
- 必须告知用户其获得源代码的权利

---

感谢您帮助保持 DMPool 的安全性！🔒
