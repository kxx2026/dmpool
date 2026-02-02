# DMPool 管理后台 - 安全优先极简方案

**原则**: 安全第一、易于实现、渐进开发

---

## 一、安全设计 (最高优先级)

### 1.1 认证与授权

**方案**: 复用现有 API 认证机制

```rust
// 使用现有的 auth_user + auth_token
// 不创建新的认证系统
 Authorization: Basic <base64(user:token)>
```

**安全措施**:
- ✅ 所有 API 必须认证
- ✅ 拒绝无认证请求
- ✅ HTTPS 强制 (生产环境)
- ✅ IP 白名单 (可选)

### 1.2 操作安全

**关键操作二次确认**:
```
┌─────────────────────────────────────┐
│  ⚠️ 确认操作                         │
│  ─────────────────────────────────│
│  您即将更改 pplns_ttl_days         │
│                                     │
│  当前值: 1                          │
│  新值: 7                            │
│                                     │
│  此操作需要重启服务                 │
│                                     │
│  [取消]  [确认并重启]                │
└─────────────────────────────────────┘
```

**操作日志**:
```
时间        用户      操作              结果
────────────────────────────────────────────
12:34:56    admin     修改start_diff   成功: 32
12:35:12    admin     修改TTL          失败: 需重启
12:36:00    admin     封禁矿工         成功: bc1q...
```

### 1.3 配置安全警告

**永远显示的关键警告**:

```
┌─────────────────────────────────────────────────────────────┐
│  🚨 安全警告 - 当前配置存在严重问题                          │
├─────────────────────────────────────────────────────────────┤
│  ❌ donation = 10000 (100%)                                  │
│     矿工将无法获得任何收益！必须立即修复。                    │
│     [立即修复] [了解更多]                                    │
├─────────────────────────────────────────────────────────────┤
│  ❌ pplns_ttl_days = 1                                       │
│     份额1天过期，矿工损失约85%收益。建议设置为7天。              │
│     [修复] [忽略]                                            │
├─────────────────────────────────────────────────────────────┤
│  ❌ ignore_difficulty = true                                  │
│     跳过难度验证，可能导致不公平收益分配。                      │
│     [修复] [忽略]                                            │
└─────────────────────────────────────────────────────────────┘
```

---

## 二、极简实现方案

### 2.1 技术选型 (最简单)

| 组件 | 方案 | 理由 |
|-----|------|------|
| **前端** | 纯 HTML + JS (无框架) | 无需构建，直接部署 |
| **样式** | 内联 CSS 或 Tailwind CDN | 无额外依赖 |
| **图表** | 纯文本/表格 (第一版) | 后期可加 Chart.js |
| **后端** | 独立二进制 `dmpool_admin` | 复用现有代码结构 |
| **通信** | REST API (JSON) | 简单成熟 |

### 2.2 部署架构

```
┌─────────────────────────────────────────────────────────────┐
│  现有架构                                                    │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                   │
│  │ DMPool   │  │  API     │  │  Grafana │                   │
│  │  Main    │  │  Server  │  │          │                   │
│  │  (3333)  │  │  (46884) │  │  (3000)   │                   │
│  └──────────┘  └──────────┘  └──────────┘                   │
│                    ↑                                        │
└────────────────────┼────────────────────────────────────────┘
                     │
              ┌────────▼─────────────┐
              │  新增: Admin Panel   │
              │  (独立二进制)         │
              │  (8080)              │
              │                      │
              │  ┌──────────────┐   │
              │  │ HTML 静态文件  │   │
              │  └──────────────┘   │
              │                      │
              │  安全:              │
              │  - Basic Auth       │
              │  - HTTPS 推荐      │
              │  - IP 白名单       │
              └─────────────────────┘
```

### 2.3 功能优先级 (MVP)

#### 第一版 (1周) - 只读 + 警告

```
✅ 必须有:
  - 配置查看 (只读)
  - 关键参数警告
  - 基础状态显示

⏸️ 暂不做:
  - 配置修改 (先看后改)
  - 复杂图表
  - 矿工管理
```

#### 第二版 (1周) - 安全配置修改

```
✅ 新增:
  - 安全参数的热更新
  - 操作日志
  - 配置导出

⏸️ 暂不做:
  - 需要重启的配置修改
  - 矿工管理
```

#### 第三版 (2周) - 完整功能

```
✅ 新增:
  - 所有配置修改
  - 重启控制
  - 矿工管理

✅ 新增:
  - 实时监控图表
  - 区块记录
```

---

## 三、第一版详细设计 (1周实现)

### 3.1 页面结构

**单页面应用** (`index.html`):

```html
<!DOCTYPE html>
<html>
<head>
    <title>DMPool 管理后台</title>
    <style>
        /* 内联样式 - 最简单 */
        body { background: #1a1a1a; color: #e0e0e0; font-family: monospace; }
        .container { max-width: 1200px; margin: 0 auto; padding: 20px; }
        .warning { background: #3a1a1a; border: 1px solid #ff4444; padding: 15px; margin: 20px 0; }
        .warning.critical { background: #4a1a1a; border-color: #ff0000; }
        .section { background: #2a2a2a; padding: 20px; margin: 20px 0; border-radius: 8px; }
        .ok { color: #00ff00; }
        .error { color: #ff0000; }
        table { width: 100%; border-collapse: collapse; }
        th, td { padding: 10px; text-align: left; border-bottom: 1px solid #3a3a3a; }
        .refresh { position: fixed; top: 20px; right: 20px; }
    </style>
</head>
<body>
    <div class="container">
        <h1>DMPool 管理后台</h1>
        <p>状态: <span id="status">检查中...</span></p>

        <!-- 安全警告区 - 始终显示 -->
        <div id="warnings"></div>

        <!-- 配置查看 -->
        <div class="section">
            <h2>配置参数</h2>
            <table id="configTable"></table>
        </div>

        <!-- 基础状态 -->
        <div class="section">
            <h2>矿池状态</h2>
            <table id="statusTable"></table>
        </div>

        <button class="refresh" onclick="loadData()">🔄 刷新</button>
    </div>

    <script>
        // 最简单的 JS 实现
        const API = '/api';
        const AUTH = getAuth();

        function getAuth() {
            let user = localStorage.getItem('dmpool_user');
            let token = localStorage.getItem('dmpool_token');
            if (!user || !token) {
                user = prompt('用户名:');
                token = prompt('认证令牌:');
                localStorage.setItem('dmpool_user', user);
                localStorage.setItem('dmpool_token', token);
            }
            return 'Basic ' + btoa(user + ':' + token);
        }

        async function loadData() {
            try {
                // 加载配置
                const configResp = await fetch(API + '/config', {
                    headers: { 'Authorization': AUTH }
                });
                const config = await configResp.json();
                displayConfig(config);

                // 加载状态
                const statusResp = await fetch(API + '/status', {
                    headers: { 'Authorization': AUTH }
                });
                const status = await statusResp.json();
                displayStatus(status);

                // 检查警告
                checkWarnings(config);

                document.getElementById('status').innerHTML = '<span class="ok">● 运行中</span>';
            } catch (e) {
                document.getElementById('status').innerHTML = '<span class="error">✗ 连接失败</span>';
                console.error(e);
            }
        }

        function displayConfig(config) {
            const table = document.getElementById('configTable');
            const items = [
                ['Stratum 端口', config.stratum_port],
                ['初始难度', config.start_difficulty],
                ['最低难度', config.minimum_difficulty],
                ['PPLNS TTL (天)', config.pplns_ttl_days, checkTTL],
                ['网络', config.network],
                ['矿池签名', config.pool_signature || '(无)'],
                ['难度验证', config.ignore_difficulty ? '❌ 已禁用' : '✅ 已启用'],
            ];

            table.innerHTML = items.map(([key, val, check]) => {
                if (check) {
                    const result = check(val);
                    val = result.value;
                    if (result.warning) {
                        return `<tr><td>${key}</td><td class="error">${val} ⚠️</td></tr>`;
                    }
                }
                return `<tr><td>${key}</td><td>${val}</td></tr>`;
            }).join('');
        }

        function checkTTL(days) {
            if (days < 7) {
                return { value: `${days} (太短!)`, warning: true };
            }
            return { value: `${days} (标准)` };
        }

        function checkWarnings(config) {
            const warnings = document.getElementById('warnings');
            const items = [];

            if (config.ignore_difficulty) {
                items.push({
                    level: 'critical',
                    text: '❌ ignore_difficulty = true - 已禁用难度验证！可能导致不公平收益分配。建议设置为 false。'
                });
            }

            if (config.pplns_ttl_days < 7) {
                items.push({
                    level: 'critical',
                    text: `❌ pplns_ttl_days = ${config.pplns_ttl_days} - 份额过期太快！矿工可能损失约${Math.floor((7-config.pplns_ttl_days)/7*100)}%收益。建议设置为 7。`
                });
            }

            if (items.length === 0) {
                warnings.innerHTML = '<p class="ok">✅ 所有配置正常</p>';
            } else {
                warnings.innerHTML = '<div class="warning critical">' +
                    '<h3>🚨 发现严重配置问题：</h3>' +
                    items.map(w => `<p>${w.text}</p>`).join('') +
                    '</div>';
            }
        }

        // 页面加载时获取数据
        loadData();
        // 每30秒自动刷新
        setInterval(loadData, 30000);
    </script>
</body>
</html>
```

### 3.2 后端 API (复用现有)

```rust
// src/api/admin.rs - 新增

use axum::{Json, Router};
use p2poolv2_lib::config::Config;

pub fn admin_router() -> Router {
    Router::new()
        .route("/config", get(get_config))
        .route("/status", get(get_status))
        .route("/health", get(health_check))
}

// 获取配置 (只读)
async fn get_config() -> Json<ConfigView> {
    let config = load_config().await?;
    Ok(Json(ConfigView::from(config)))
}

// 获取状态
async fn get_status() -> Json<PoolStatus> {
    Ok(Json(PoolStatus {
        uptime: get_uptime(),
        connections: get_connection_count(),
        shares_per_sec: get_share_rate(),
    }))
}
```

### 3.3 安全检查函数

```rust
// src/api/safety.rs - 安全验证

/// 检查配置是否有严重安全问题
pub fn check_safety(config: &Config) -> SafetyReport {
    let mut issues = vec![];

    // 检查1: donation
    if let Some(donation) = config.stratum.donation {
        if donation >= 10000 {
            issues.push(SafetyIssue {
                severity: Severity::Critical,
                param: "donation",
                message: "donation = 10000 意味着矿工收益为零！",
                fix: "设置为 0 或注释掉 donation",
            });
        }
    }

    // 检查2: pplns_ttl_days
    if config.store.pplns_ttl_days < 7 {
        issues.push(SafetyIssue {
            severity: Severity::Critical,
            param: "pplns_ttl_days",
            message: format!("TTL={} 太短，矿工损失收益", config.store.pplns_ttl_days),
            fix: "设置为 7",
        });
    }

    // 检查3: ignore_difficulty
    if config.stratum.ignore_difficulty.unwrap_or(false) {
        issues.push(SafetyIssue {
            severity: Severity::Critical,
            param: "ignore_difficulty",
            message: "跳过难度验证，收益分配可能不公平",
            fix: "设置为 false",
        });
    }

    SafetyReport { issues }
}
```

---

## 四、实现步骤 (1周)

### Day 1-2: 基础架构

1. ✅ 创建 `src/bin/dmpool_admin.rs`
2. ✅ 添加到 `Cargo.toml` 的 `[[bin]]`
3. ✅ 实现基础 API 端点 (`/config`, `/status`)
4. ✅ 创建 `static/admin/index.html`

### Day 3-4: 安全检查

5. ✅ 实现 `check_safety()` 函数
6. ✅ 添加警告显示逻辑
7. ✅ 测试各种错误配置

### Day 5: 测试与文档

8. ✅ 本地测试
9. ✅ 编写使用文档
10. ✅ 安全审查

---

## 五、部署配置

### 5.1 systemd 服务

```ini
# /etc/systemd/system/dmpool-admin.service
[Unit]
Description=DMPool Admin Panel
After=dmpool.service
Requires=dmpool.service

[Service]
Type=simple
User=dmpool
Group=dmpool
WorkingDirectory=/etc/dmpool
Environment="CONFIG_PATH=/etc/dmpool/config.toml"
Environment="ADMIN_PORT=8080"
ExecStart=/usr/local/bin/dmpool_admin
Restart=always

[Install]
WantedBy=multi-user.target
```

### 5.2 Nginx 反向代理 (推荐)

```nginx
# /etc/nginx/sites-available/dmpool-admin
server {
    listen 443 ssl http2;
    server_name admin.dmpool.org;

    ssl_certificate /etc/letsencrypt/live/admin.dmpool.org/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/admin.dmpool.org/privkey.pem;

    # IP 白名单 (推荐)
    allow 192.168.1.0/24;  # 办公室IP
    deny all;

    location / {
        auth_basic "DMPool Admin";
        auth_basic_user_file /etc/nginx/.htpasswd;

        proxy_pass http://localhost:8080;
    }
}
```

---

## 六、使用说明

### 6.1 访问地址

```
开发环境:  http://localhost:8080
生产环境:  https://admin.dmpool.org
```

### 6.2 首次使用

```
1. 访问管理后台
2. 输入用户名和令牌 (从 config.toml 获取)
3. 查看配置警告
4. 根据警告修改配置
```

### 6.3 安全检查清单

部署前确认：
- [ ] HTTPS 已配置
- [ ] IP 白名单已设置
- [ ] 默认凭证已更改
- [ ] 防火墙规则已配置
- [ ] 操作日志已启用

---

## 七、总结

### 7.1 设计原则

| 原则 | 说明 |
|-----|------|
| **安全第一** | 所有操作需要认证，关键操作有确认 |
| **简单可靠** | 无框架依赖，代码易审查 |
| **渐进开发** | 第一版只读，后续再添加写入功能 |
| **明确警告** | 配置问题有清晰的风险提示 |

### 7.3 第一版功能

✅ 配置查看
✅ 安全警告
✅ 基础状态显示
✅ 操作日志
✅ Basic Auth 认证

### 7.4 不在第一版

❌ 配置修改 (第二版)
❌ 矿工管理 (第三版)
❌ 图表可视化 (第三版)
❌ 高级功能 (后续)

---

**下一步**: 开始实现第一版 (1周完成)
