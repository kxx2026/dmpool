# DMPool Admin API Documentation

This document describes the DMPool Admin API for managing the Bitcoin mining pool.

## Table of Contents

- [Authentication](#authentication)
- [Rate Limiting](#rate-limiting)
- [API Endpoints](#api-endpoints)
- [Error Codes](#error-codes)
- [Web Interface](#web-interface)

## Authentication

The admin API uses JWT (JSON Web Token) authentication. To access protected endpoints:

1. Login to obtain a JWT token
2. Include the token in the `Authorization` header

### Login Request

```bash
POST /api/auth/login
Content-Type: application/json

{
  "username": "admin",
  "password": "your_password"
}
```

### Login Response

```json
{
  "status": "ok",
  "data": {
    "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    "user": {
      "username": "admin",
      "role": "admin"
    }
  },
  "timestamp": 1704067200
}
```

### Using the Token

Include the token in subsequent requests:

```bash
GET /api/dashboard
Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
```

## Rate Limiting

The API implements rate limiting to prevent abuse:

- **Standard endpoints**: 60 requests per minute
- **Login endpoint**: 10 requests per minute

Rate limit headers are included in responses:

```
X-RateLimit-Limit: 60
X-RateLimit-Remaining: 45
X-RateLimit-Reset: 1704070800
```

## API Endpoints

### Dashboard

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/dashboard` | Get pool metrics and statistics |

### Configuration

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/config` | Get current configuration |
| POST | `/api/config` | Update configuration |
| POST | `/api/config/reload` | Reload from config file |
| GET | `/api/config/confirmations` | List pending changes |
| POST | `/api/config/confirmations/{id}` | Confirm a change |
| POST | `/api/config/confirmations/{id}/apply` | Apply a confirmed change |

### Workers

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/workers` | List workers (paginated) |
| GET | `/api/workers/{address}` | Get worker details |
| POST | `/api/workers/{address}/ban` | Ban a worker |
| POST | `/api/workers/{address}/unban` | Unban a worker |
| POST | `/api/workers/{address}/tags` | Add tag to worker |
| POST | `/api/workers/{address}/tags/{tag}` | Remove tag from worker |

### Audit

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/audit/logs` | Get audit logs |
| GET | `/api/audit/stats` | Get audit statistics |

### Backup

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/backup/create` | Create a backup |
| GET | `/api/backup/list` | List all backups |
| GET | `/api/backup/stats` | Get backup statistics |
| GET | `/api/backup/{id}` | Get backup details |
| POST | `/api/backup/{id}/delete` | Delete a backup |
| POST | `/api/backup/{id}/restore` | Restore from backup |
| POST | `/api/backup/cleanup` | Delete old backups |

### Health

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/health` | Health check |
| GET | `/api/services/status` | Services status |

## Worker List Parameters

The `/api/workers` endpoint supports the following query parameters:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `page` | integer | 1 | Page number (min: 1) |
| `page_size` | integer | 20 | Items per page (max: 100) |
| `search` | string | - | Search by address or worker name |
| `status` | string | - | Filter: active, inactive, banned |
| `sort_by` | string | last_seen | Sort field: address, hashrate, shares, last_seen |
| `sort_order` | string | desc | Sort order: asc, desc |

### Example: Get Active Workers

```bash
GET /api/workers?status=active&sort_by=hashrate&sort_order=desc&page=1&page_size=50
```

## Error Codes

| Code | Description |
|------|-------------|
| 200 | Success |
| 400 | Bad Request - Invalid parameters |
| 401 | Unauthorized - Invalid or missing token |
| 404 | Not Found - Resource doesn't exist |
| 429 | Too Many Requests - Rate limit exceeded |
| 500 | Internal Server Error |

### Error Response Format

```json
{
  "status": "error",
  "message": "Invalid or missing authentication token",
  "timestamp": 1704067200
}
```

## Configuration Change Risk Levels

The admin system has risk levels for configuration changes:

### Critical
- `pplns_ttl_days` - TTL < 7 days causes miner loss
- `donation` - donation = 10000 means 100% donation (zero payout)
- `ignore_difficulty` - Disabling difficulty validation is dangerous

### Medium
- `start_difficulty` - Affects miner connection difficulty
- `minimum_difficulty` - Affects miner minimum difficulty

### Low
- `pool_signature` - Affects payout identification

Critical and Medium changes require explicit confirmation before being applied.

## Security Best Practices

1. **Use strong JWT secrets** - Set `JWT_SECRET` environment variable
2. **Change default credentials** - Update admin username/password
3. **Enable HTTPS** - Use reverse proxy (nginx) with SSL in production
4. **Monitor audit logs** - Regularly review `/api/audit/logs`
5. **Backup regularly** - Use `/api/backup/create` before config changes

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `CONFIG_PATH` | Path to config.toml | config.toml |
| `ADMIN_PORT` | Admin server port | 8080 |
| `ADMIN_USERNAME` | Default admin username | admin |
| `ADMIN_PASSWORD` | Default admin password | admin123 |
| `JWT_SECRET` | JWT signing secret | CHANGE_THIS_... |

## Development

### Running the Admin Server

```bash
# Set environment variables
export ADMIN_USERNAME=admin
export ADMIN_PASSWORD=secure_password
export JWT_SECRET=your_jwt_secret_here

# Run the admin server
cargo run --bin dmpool_admin
```

The admin panel will be available at `http://localhost:8080`

### OpenAPI Specification

See [openapi.yaml](openapi.yaml) for the complete API specification in OpenAPI 3.0 format.

You can use tools like:
- [Swagger UI](https://swagger.io/tools/swagger-ui/) to visualize the API
- [Redoc](https://github.com/Redocly/redoc) for beautiful documentation
- [Postman](https://www.postman.com/) for API testing

## License

This project is licensed under AGPLv3. See LICENSE for details.
