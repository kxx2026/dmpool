# DMPool

<div align="center">

**DMPool** — A fork of Hydrapool, adapted for custom mining pool operations.

[![Rust](https://img.shields.io/badge/rust-1.88.0+-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-AGPLv3-blue.svg)](./LICENSE)
[![GitHub](https://img.shields.io/badge/source-kxx2026%2Fdmpool-green.svg)](https://github.com/kxx2026/dmpool)
[![Forked from](https://img.shields.io/badge/forked%20from-256--Foundation%2FHydra--Pool-informational.svg)](https://github.com/256-Foundation/Hydra-Pool)

</div>

## About This Fork

**DMPool** is a derivative work based on [Hydrapool](https://github.com/256-Foundation/Hydra-Pool), originally developed by [256 Foundation](https://github.com/256-Foundation).

This fork maintains compatibility with the original project while being customized for specific mining pool operations. All modifications are released under the same **AGPLv3** license as required by the original project.

### Original Project

- **Project**: [Hydrapool](https://github.com/256-Foundation/Hydra-Pool)
- **Authors**: 256 Foundation (Kulpreet Singh)
- **License**: AGPLv3

### This Fork

- **Repository**: [kxx2026/dmpool](https://github.com/kxx2026/dmpool)
- **Forked from**: 256-Foundation/Hydra-Pool
- **License**: AGPLv3 (inherited from original)

## Overview

DMPool enables you to run your own Bitcoin mining pool with **zero custody** — all payouts are made directly from the coinbase transaction. Pool operators never touch user funds.

### Key Features

| Feature | Description |
|---------|-------------|
| **Non-Custodial** | Payouts directly from coinbase, no trust required |
| **PPLNS Accounting** | Fair reward distribution based on contributed shares |
| **Transparent** | Public API for share verification and payout auditing |
| **Monitoring** | Integrated Prometheus/Grafana dashboards |
| **Compatible** | Works with any Bitcoin RPC node |
| **Extensible** | Built in Rust for easy customization |

## Quick Start

### Docker (Recommended)

```bash
# Download configurations
curl -fsSL https://github.com/kxx2026/dmpool/releases/latest/download/docker-compose.yml -o docker-compose.yml
curl -fsSL https://github.com/kxx2026/dmpool/releases/latest/download/config-example.toml -o config.toml

# Edit config.toml with your Bitcoin node details
nano config.toml

# Start the pool
docker compose up -d
```

Services will be available at:
- **Stratum**: `stratum://localhost:3333`
- **API**: `http://localhost:46884`
- **Dashboard**: `http://localhost:3000`

### Binary Installation

```bash
curl -fsSL https://github.com/kxx2026/dmpool/releases/latest/download/dmpool-installer.sh | sh
```

## Configuration

Edit `config.toml`:

```toml
[store]
path = "./store.db"
pplns_ttl_days = 1

[stratum]
hostname = "0.0.0.0"
port = 3333
bootstrap_address = "bc1q...your_address"
zmqpubhashblock = "tcp://127.0.0.1:28334"
network = "main"
pool_signature = "dmpool"

[bitcoinrpc]
url = "http://127.0.0.1:8332"
username = "bitcoin"
password = "your_rpc_password"

[api]
hostname = "0.0.0.0"
port = 46884
auth_user = "dmpool"
auth_token = "generated_token"
```

Generate authentication token:

```bash
dmpool_cli gen-auth <username> <password>
```

## Building from Source

```bash
# Install dependencies (Ubuntu/Debian)
sudo apt install -y libssl-dev pkg-config clang libclang-dev

# Clone and build
git clone https://github.com/kxx2026/dmpool.git
cd dmpool
cargo build --release

# Run
./target/release/dmpool
```

**Requirements**: Rust 1.88.0+

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         Miners                              │
│  (stratum://pool:3333)                                      │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                      DMPool Core                            │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────────┐   │
│  │  Stratum    │  │   PPLNS      │  │   Coinbase      │   │
│  │  Server     │─▶│   Engine     │─▶│   Builder       │   │
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

## API Endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /health` | Health check |
| `GET /pplns_shares` | Download all PPLNS shares (JSON) |
| `GET /pplns_shares?start_time=X&end_time=Y` | Filtered shares |

## Monitoring

Built-in monitoring with Prometheus and Grafana:

```bash
docker compose up -d prometheus grafana
```

Dashboards include:
- Pool hashrate and shares per second
- User and worker statistics
- Difficulty tracking
- Uptime monitoring

## Bitcoin Node Configuration

Adjust `blockmaxweight` in `bitcoin.conf` to reserve space for coinbase outputs:

```ini
# Reserve space for ~500 P2PKH outputs
blockmaxweight=3930000
```

| Outputs | Coinbase Weight | Recommended `blockmaxweight` |
|---------|-----------------|------------------------------|
| 100     | ~13,808 WU      | 3,986,000                    |
| 500     | ~68,208 WU      | 3,930,000                    |
| 1,000   | ~136,208 WU     | 3,860,000                    |

## Security

- **API Authentication**: Configure `auth_user` and `auth_token`
- **Firewall**: Restrict API access to trusted IPs
- **HTTPS**: Use nginx/Caddy as reverse proxy for public dashboards
- **Updates**: Monitor releases for security patches

## Documentation

- [Deployment Guide](./DEPLOYMENT.md) — Production deployment
- [Changelog](./CHANGELOG.md) — Version history

## License

This project is licensed under **AGPLv3** — see [LICENSE](./LICENSE) for details.

This is a derivative work based on [Hydrapool](https://github.com/256-Foundation/Hydra-Pool) by 256 Foundation, originally licensed under AGPLv3. This fork maintains the same license in compliance with the original project's requirements.

### AGPLv3 Summary

This license requires:
- **Source Availability**: If you run this software on a server and provide services to users, you must make the source code available to those users
- **Share Alike**: Any modifications must be released under the same AGPLv3 license
- **Attribution**: Credit must be given to the original authors (256 Foundation)
- **State Changes**: Any significant modifications to the code must be noted

## Contributing

Contributions are welcome! Please:
1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Submit a pull request

## Acknowledgments

- **Original Project**: [Hydrapool](https://github.com/256-Foundation/Hydra-Pool) by 256 Foundation
- **Core Dependencies**: [p2poolv2](https://github.com/p2poolv2/p2poolv2)

## Support

- **Issues**: [GitHub Issues](https://github.com/kxx2026/dmpool/issues)
- **Discussions**: [GitHub Discussions](https://github.com/kxx2026/dmpool/discussions)

---

**DMPool** — A fork of Hydrapool, adapted for custom mining pool operations
