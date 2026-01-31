# Changelog

All notable changes to DMPool will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **Note**: DMPool is a fork of [Hydrapool](https://github.com/256-Foundation/Hydra-Pool) 
> by 256 Foundation. Changes documented here are specific to this fork.

## [Unreleased]

### Added
- Chinese (zh-CN) localization support
- Chinese Grafana dashboards
- Proper attribution to original Hydrapool project
- AGPLv3 compliance documentation

### Changed
- Project renamed to DMPool
- Repository relocated to kxx2026/dmpool
- All documentation updated with fork attribution

## [2.4.0] - 2024-12-XX

### Added
- Job tracker and emission optimizations
- Enhanced web API for Prometheus integration
- Improved logging and statistics aggregation

### Changed
- Database schema for better performance
- Configuration structure updates

## [2.2.2] - 2024-XX-XX

### Added
- Web API enabled for Prometheus metrics
- 15-second scrape interval optimization

### Fixed
- Notify send performance issues
- Stats backup improvements

---

## Version History

| Version | Date | Description |
|---------|------|-------------|
| 2.4.0 | TBD | Performance optimizations, Chinese localization |
| 2.2.2 | 2024 | Prometheus integration, scrape interval tuning |
| 2.x.x | 2024 | Initial PPLNS implementation |

---

## Migration Guide

### Upgrading from v1.x to v2.x

**Breaking Changes**: Database schema has changed.

```bash
# Backup existing data
cp store.db store.db.backup

# Remove old database
rm store.db

# Start new version
dmpool
```

### Upgrading from v2.2.x to v2.4.0

No database migration required. Simply update the binary:

```bash
# Docker
docker compose pull
docker compose up -d

# Binary
wget https://github.com/kxx2026/dmpool/releases/latest/download/dmpool
sudo mv dmpool /usr/local/bin/
sudo systemctl restart dmpool
```

### Migrating from Hydrapool to DMPool

DMPool maintains compatibility with Hydrapool configurations. To migrate:

1. Backup your existing Hydrapool installation
2. Install DMPool (see README.md for installation methods)
3. Copy your `config.toml` from Hydrapool to DMPool
4. Update any hardcoded paths if necessary
5. Start DMPool

**Note**: The database format is compatible, but backup before migration.

---

## Contributing

To report a bug or request a feature, please [open an issue](https://github.com/kxx2026/dmpool/issues).

When contributing, please note that:
- All contributions must be licensed under AGPLv3
- Attribution to the original Hydrapool project should be maintained
- Significant changes should be documented in this changelog

---

## Links

- **This Fork**: [GitHub Repository](https://github.com/kxx2026/dmpool)
- **Original Project**: [256-Foundation/Hydra-Pool](https://github.com/256-Foundation/Hydra-Pool)
- **Releases**: [GitHub Releases](https://github.com/kxx2026/dmpool/releases)
- **Documentation**: [README.md](./README.md)
