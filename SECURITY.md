# Security Policy

## Supported Versions

Currently, security updates are provided for the latest release:

| Version | Supported |
|---------|-----------|
| 2.4.x   | ✅        |
| < 2.4.0 | ❌        |

## Reporting a Vulnerability

**DO NOT** file a public issue for security vulnerabilities.

### How to Report

Send an email to: **security@dmpool.org** (or create a [GitHub Security Advisory](https://github.com/kxx2026/dmpool/security/advisories))

Please include:
- **Description**: Clear description of the vulnerability
- **Impact**: Potential impact of the vulnerability
- **Reproduction**: Steps to reproduce (if applicable)
- **Proof of Concept**: Any code or screenshots demonstrating the issue

### What to Expect

1. **Confirmation**: We will acknowledge receipt within 48 hours
2. **Assessment**: We will assess the severity and determine a fix timeline
3. **Coordination**: We will work with you on a coordinated disclosure
4. **Disclosure**: We will disclose the vulnerability once a fix is released

### Security Best Practices for Operators

When deploying DMPool in production:

#### 1. API Authentication

```toml
[api]
auth_user = "admin"
auth_token = "GENERATE_STRONG_TOKEN"
```

Generate a secure token:
```bash
dmpool_cli gen-auth admin "STRONG_PASSWORD_32_CHARS+"
```

#### 2. Network Security

```bash
# Firewall - only allow necessary ports
sudo ufw allow 3333/tcp  # Stratum
sudo ufw allow 22/tcp    # SSH
sudo ufw enable

# API should be internal only
# Use nginx reverse proxy for external access with rate limiting
```

#### 3. Bitcoin RPC Security

```ini
# bitcoin.conf
rpcuser=CHANGE_THIS
rpcpassword=CHANGE_THIS_STRONG_PASSWORD
rpcallowip=127.0.0.1
rpcbind=127.0.0.1
```

#### 4. System Hardening

```bash
# Run as non-root user
sudo useradd -r -s /bin/false dmpool

# Configure fail2ban
sudo apt install fail2ban

# Keep system updated
sudo unattended-upgrades
```

#### 5. Regular Audits

- Review access logs weekly
- Monitor for unusual hashrate patterns
- Keep dependencies updated
- Review Prometheus alerts

### AGPLv3 Security Obligations

As required by AGPLv3:
- If you modify the software and run it as a network service, you must make the source code available to users
- Any security fixes you make must be shared back to the community
- Users must be informed of their right to receive source code

---

Thank you for helping keep DMPool secure! 🔒
