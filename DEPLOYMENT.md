# DMPool Production Deployment Guide

<div align="center">

**Production-ready deployment guide for DMPool Bitcoin mining pool**

*A fork of Hydrapool by 256 Foundation*

</div>

> **Note**: DMPool is a derivative work based on [Hydrapool](https://github.com/256-Foundation/Hydra-Pool), originally developed by 256 Foundation and licensed under AGPLv3. This guide is adapted for the forked version.

## Table of Contents

1. [System Requirements](#system-requirements)
2. [Network Architecture](#network-architecture)
3. [Installation](#installation)
4. [Configuration](#configuration)
5. [Monitoring](#monitoring)
6. [Security Hardening](#security-hardening)
7. [Backup Strategy](#backup-strategy)
8. [Maintenance](#maintenance)
9. [Troubleshooting](#troubleshooting)
10. [Compliance](#compliance)

---

## System Requirements

### Hardware Specifications

| Tier | CPU | RAM | Storage | Network | Max Miners |
|------|-----|-----|---------|---------|------------|
| **Minimum** | 2 cores @ 2.0 GHz | 2 GB | 20 GB SSD | 100 Mbps | ~10 |
| **Standard** | 4 cores @ 2.5 GHz | 8 GB | 100 GB NVMe | 1 Gbps | ~50 |
| **High Performance** | 8 cores @ 3.0 GHz | 16 GB | 500 GB NVMe | 10 Gbps | ~200 |
| **Enterprise** | 16+ cores @ 3.5 GHz | 32 GB | 2 TB NVMe RAID10 | 10 Gbps | 500+ |

### Operating System

- **Recommended**: Ubuntu 24.04 LTS
- **Supported**: Debian 12+, Rocky Linux 9+, Arch Linux

### Software Dependencies

```bash
# Bitcoin Core (24.0+)
sudo apt install -y bitcoind

# Docker & Docker Compose (for containerized deployment)
curl -fsSL https://get.docker.com | sh

# Rust 1.88.0+ (for building from source)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

---

## Network Architecture

### Port Layout

| Port | Service | Public Exposure |
|------|---------|-----------------|
| 3333 | Stratum (Mining) | Yes |
| 46884 | API Server | No (VPN/Internal) |
| 3000 | Grafana Dashboard | No (VPN/Internal) |
| 9090 | Prometheus | No (Internal only) |
| 28334 | ZMQ (Bitcoin → Pool) | No (Local) |

### Firewall Rules

```bash
# UFW (Ubuntu)
sudo ufw allow 3333/tcp comment 'DMPool Stratum'
sudo ufw allow 22/tcp comment 'SSH'
sudo ufw enable

# iptables
iptables -A INPUT -p tcp --dport 3333 -j ACCEPT
iptables -A INPUT -p tcp --dport 22 -j ACCEPT
iptables -P INPUT DROP
```

---

## Installation

### Method 1: Docker (Recommended)

```bash
# 1. Download configs
curl -fsSL https://github.com/kxx2026/dmpool/releases/latest/download/docker-compose.yml -o docker-compose.yml
curl -fsSL https://github.com/kxx2026/dmpool/releases/latest/download/config-example.toml -o config.toml

# 2. Configure
nano config.toml

# 3. Start services
docker compose up -d

# 4. Verify
docker compose ps
docker compose logs -f dmpool
```

### Method 2: Debian Package

```bash
# Download and install
wget https://github.com/kxx2026/dmpool/releases/latest/download/dmpool_2.4.0_amd64.deb
sudo dpkg -i dmpool_2.4.0_amd64.deb

# Configure
sudo nano /etc/dmpool/config.toml

# Start service
sudo systemctl enable dmpool
sudo systemctl start dmpool
```

### Method 3: Build from Source

```bash
# Install build dependencies
sudo apt install -y libssl-dev pkg-config clang libclang-dev

# Clone and build
git clone https://github.com/kxx2026/dmpool.git
cd dmpool
cargo build --release

# Install
sudo cp target/release/dmpool /usr/local/bin/
sudo cp target/release/dmpool_cli /usr/local/bin/
sudo cp config.toml /etc/dmpool/
```

---

## Configuration

### Bitcoin RPC Setup

```ini
# bitcoin.conf
server=1
rest=1
rpcuser=dmpool_rpc
rpcpassword=CHANGE_THIS_SECURE_PASSWORD
rpcallowip=127.0.0.1
rpcbind=0.0.0.0
zmqpubhashblock=tcp://0.0.0.0:28334

# Reserve space for coinbase (500 outputs)
blockmaxweight=3930000
```

### DMPool Configuration

```toml
# /etc/dmpool/config.toml
[store]
path = "/var/lib/dmpool/store.db"
background_task_frequency_hours = 24
pplns_ttl_days = 1

[stratum]
hostname = "0.0.0.0"
port = 3333
start_difficulty = 1
minimum_difficulty = 1
bootstrap_address = "bc1qYOUR_ADDRESS"
donation_address = "bc1qDONATION_ADDRESS"
donation = 100  # 1% (100 basis points)
zmqpubhashblock = "tcp://127.0.0.1:28334"
network = "main"
pool_signature = "dmpool"

[bitcoinrpc]
url = "http://127.0.0.1:8332"
username = "dmpool_rpc"
password = "YOUR_SECURE_PASSWORD"

[logging]
level = "info"
stats_dir = "/var/log/dmpool/stats"

[api]
hostname = "127.0.0.1"
port = 46884
auth_user = "admin"
auth_token = "GENERATE_WITH_dmpool_cli"
```

### Generate Authentication Token

```bash
dmpool_cli gen-auth admin YOUR_PASSWORD
```

---

## Monitoring

### Prometheus + Grafana Setup

```bash
# Add to docker-compose.yml
services:
  prometheus:
    image: prom/prometheus:latest
    volumes:
      - ./prometheus/prometheus.yml:/etc/prometheus/prometheus.yml
    ports:
      - "9090:9090"

  grafana:
    image: grafana/grafana:latest
    volumes:
      - grafana-storage:/var/lib/grafana
      - ./grafana/provisioning:/etc/grafana/provisioning
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=CHANGE_ME
```

### Key Metrics to Monitor

| Metric | Description | Alert Threshold |
|--------|-------------|-----------------|
| Pool Hashrate | Total mining power | < 50% of expected |
| Share Rate | Shares per second | < 1/s for 5min |
| API Latency | Response time | > 500ms |
| DB Size | Database growth | > 10 GB/day |
| Connection Count | Active miners | Unexpected drop |

---

## Security Hardening

### 1. System Hardening

```bash
# Disable root login
sudo sed -i 's/PermitRootLogin yes/PermitRootLogin no/' /etc/ssh/sshd_config

# Configure fail2ban
sudo apt install fail2ban
sudo systemctl enable fail2ban

# Enable automatic updates
sudo apt install unattended-upgrades
sudo dpkg-reconfigure -plow unattended-upgrades
```

### 2. Application Security

```bash
# Run as non-root user
sudo useradd -r -s /bin/false dmpool
sudo chown -R dmpool:dmpool /etc/dmpool /var/lib/dmpool /var/log/dmpool

# Systemd service with drop privileges
# /etc/systemd/system/dmpool.service
[Unit]
Description=DMPool Bitcoin Mining Pool (Fork of Hydrapool)
After=network.target bitcoin.service

[Service]
Type=simple
User=dmpool
Group=dmpool
WorkingDirectory=/etc/dmpool
ExecStart=/usr/local/bin/dmpool
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

### 3. Network Security

```bash
# Rate limiting with nginx
limit_req_zone \ zone=api_limit:10m rate=10r/s;

server {
    listen 443 ssl http2;
    server_name pool.example.com;

    ssl_certificate /etc/letsencrypt/live/pool.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/pool.example.com/privkey.pem;

    location /api/ {
        limit_req zone=api_limit;
        proxy_pass http://localhost:46884;
        proxy_set_header Host \;
    }
}
```

---

## Backup Strategy

### Automated Backup Script

```bash
#!/bin/bash
# /usr/local/bin/backup-dmpool.sh

BACKUP_DIR="/backups/dmpool"
DATE=$(date +%Y%m%d_%H%M%S)

# Backup database
sudo -u dmpool cp /var/lib/dmpool/store.db "\/store_\.db"

# Backup config
cp /etc/dmpool/config.toml "\/config_\.toml"

# Compress and cleanup
find "\" -name "store_*" -mtime +7 -delete
find "\" -name "config_*" -mtime +30 -delete

echo "Backup completed: \"
```

### Offsite Backup

```bash
# Rsync to remote server
rsync -avz --delete /var/lib/dmpool/ backup-server:/backups/dmpool/

# Or use restic for encrypted backups
restic backup /var/lib/dmpool/ /etc/dmpool/
restic forget --keep-daily 7 --keep-weekly 4 --keep-monthly 12
```

---

## Maintenance

### Regular Tasks

| Frequency | Task | Command |
|-----------|------|---------|
| Daily | Check logs |  |
| Weekly | Review alerts | Check Grafana dashboards |
| Weekly | Backup verification | Test restore procedure |
| Monthly | Update software |  |
| Quarterly | Security audit | Review access logs |

### Update Procedure

```bash
# 1. Stop service
sudo systemctl stop dmpool

# 2. Backup current version
sudo cp /usr/local/bin/dmpool /usr/local/bin/dmpool.backup

# 3. Install new version
wget https://github.com/kxx2026/dmpool/releases/latest/download/dmpool
sudo chmod +x dmpool
sudo mv dmpool /usr/local/bin/

# 4. Start service
sudo systemctl start dmpool

# 5. Verify
sudo systemctl status dmpool
dmpool_cli --version
```

---

## Troubleshooting

### Common Issues

**Issue**: Miners cannot connect
```bash
# Check stratum is listening
sudo ss -nltp | grep 3333

# Check firewall
sudo ufw status

# Check logs
journalctl -u dmpool -f
```

**Issue**: No new blocks
```bash
# Verify ZMQ connection
sudo netstat -an | grep 28334

# Check Bitcoin node
bitcoin-cli getblockcount
```

**Issue**: High memory usage
```bash
# Check database size
du -sh /var/lib/dmpool/store.db

# Adjust TTL in config.toml
pplns_ttl_days = 0.5  # Reduce retention
```

### Log Locations

- **Systemd journal**: 
- **Application logs**: 
- **Bitcoin logs**: 

---

## Compliance

### AGPLv3 License Requirements

As a derivative of Hydrapool (licensed under AGPLv3), DMPool must comply with the following:

1. **Source Code Availability**: If you run DMPool as a network service, you must provide source code to users of that service
2. **License Preservation**: All modifications must be licensed under AGPLv3
3. **Attribution**: Credit to original authors (256 Foundation) must be maintained
4. **State Changes**: Significant modifications must be documented

### Original Project Attribution

```
DMPool is a fork of Hydrapool by 256 Foundation.
Original project: https://github.com/256-Foundation/Hydra-Pool
Original authors: Kulpreet Singh and contributors
License: AGPLv3
```

### Providing Source to Users

If you operate a public mining pool with DMPool, you must:

1. Include a link to the source code in your pool's website footer
2. Provide source upon request to pool users
3. Maintain a public repository with your modifications

Example footer text:
```html
Powered by DMPool, a fork of <a href="https://github.com/256-Foundation/Hydra-Pool">Hydrapool</a> 
by 256 Foundation. Source code available at <a href="https://github.com/kxx2026/dmpool">GitHub</a>.
```

---

## Support

- **Documentation**: [https://github.com/kxx2026/dmpool](https://github.com/kxx2026/dmpool)
- **Original Project**: [https://github.com/256-Foundation/Hydra-Pool](https://github.com/256-Foundation/Hydra-Pool)
- **Issues**: [GitHub Issues](https://github.com/kxx2026/dmpool/issues)
- **Community**: [GitHub Discussions](https://github.com/kxx2026/dmpool/discussions)
