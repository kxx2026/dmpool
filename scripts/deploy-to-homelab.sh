#!/bin/bash
# DMPool 部署脚本 - 用于 homelab 测试

set -e

HOST="homelab"
USER="k0n9"
DEPLOY_DIR="/home/k0n9/dmpool"
SERVICE_DIR="/home/k0n9/dmpool/services"

echo "🚀 开始部署 DMPool 到 homelab..."

# 1. 创建目录结构
ssh ${USER}@${HOST} "mkdir -p ${DEPLOY_DIR}/{data,logs,config}"

# 2. 复制二进制文件
echo "📦 上传二进制文件..."
scp target/release/dmpool ${USER}@${HOST}:${DEPLOY_DIR}/
scp target/release/dmpool_admin ${USER}@${HOST}:${DEPLOY_DIR}/

# 3. 复制配置文件
echo "⚙️  上传配置文件..."
scp docker/config-example.toml ${USER}@${HOST}:${DEPLOY_DIR}/config/config.toml

# 4. 复制服务文件
echo "📋 创建 systemd 服务..."
cat > /tmp/dmpool.service << 'EOF'
[Unit]
Description=DMPool Mining Pool
After=network.target bitcoin.service

[Service]
Type=simple
User=k0n9
WorkingDirectory=/home/k0n9/dmpool
ExecStart=/home/k0n9/dmpool/dmpool --config /home/k0n9/dmpool/config/config.toml
Restart=always
RestartSec=10

# 日志
StandardOutput=append:/home/k0n9/dmpool/logs/dmpool.log
StandardError=append:/home/k0n9/dmpool/logs/dmpool-error.log

# 安全
NoNewPrivileges=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
EOF

cat > /tmp/dmpool-admin.service << 'EOF'
[Unit]
Description=DMPool Admin Panel
After=network.target dmpool.service

[Service]
Type=simple
User=k0n9
WorkingDirectory=/home/k0n9/dmpool
Environment="CONFIG_PATH=/home/k0n9/dmpool/config/config.toml"
Environment="ADMIN_PORT=8080"
ExecStart=/home/k0n9/dmpool/dmpool_admin
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

scp /tmp/dmpool.service ${USER}@${HOST}:/tmp/
scp /tmp/dmpool-admin.service ${USER}@${HOST}:/tmp/

# 5. 安装服务
echo "🔧 安装 systemd 服务..."
ssh ${USER}@${HOST} "
sudo mv /tmp/dmpool.service /etc/systemd/system/
sudo mv /tmp/dmpool-admin.service /etc/systemd/system/
sudo systemctl daemon-reload
"

echo ""
echo "✅ 部署完成！"
echo ""
echo "下一步："
echo "1. 编辑配置: ssh ${USER}@${HOST} 'nano ${DEPLOY_DIR}/config/config.toml'"
echo "2. 启动服务: ssh ${USER}@${HOST} 'sudo systemctl start dmpool dmpool-admin'"
echo "3. 查看日志: ssh ${USER}@${HOST} 'journalctl -u dmpool -f'"
echo "4. 访问管理后台: http://192.168.5.21:8080"
echo ""
