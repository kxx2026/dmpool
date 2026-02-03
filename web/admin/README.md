# DMPool Admin Frontend

Modern Vue 3 + TypeScript admin dashboard for DMPool Bitcoin mining pool.

## Features

- **Dashboard**: Real-time pool metrics, hashrate charts, recent activity
- **Worker Management**: Paginated worker list with search, filtering, tags, and ban/unban
- **Configuration**: Pool settings with confirmation for dangerous changes
- **Audit Logs**: Security audit trail with filtering
- **Backup**: Database backup creation, listing, and restore
- **Alerts**: Alert rule configuration and history

## Tech Stack

- **Vue 3** - Progressive JavaScript framework
- **TypeScript** - Type safety
- **Naive UI** - Modern Vue 3 UI library
- **Pinia** - State management
- **Vue Router** - Routing
- **Axios** - HTTP client
- **Vite** - Build tool

## Development

### Prerequisites

- Node.js 18+
- pnpm 8+ (or npm/yarn)

### Installation

```bash
cd web/admin
pnpm install
```

### Development Server

```bash
pnpm dev
```

The admin panel will be available at `http://localhost:3000`

API requests are proxied to `http://localhost:8080` (configurable in `vite.config.ts`)

### Build for Production

```bash
pnpm build
```

The built files will be in `dist/`

### Preview Production Build

```bash
pnpm preview
```

## Configuration

Create a `.env` file (see `.env.example`):

```bash
cp .env.example .env
```

Edit `.env` with your configuration:

```env
VITE_API_BASE_URL=http://localhost:8080
VITE_API_TIMEOUT=30000
```

## Project Structure

```
src/
├── api/           # API client and types
├── assets/        # Static assets
├── components/    # Reusable Vue components
├── router/        # Vue Router configuration
├── stores/        # Pinia stores
├── types/         # TypeScript type definitions
├── utils/         # Utility functions
├── views/         # Page components
├── App.vue        # Root component
└── main.ts        # Application entry point
```

## Adding New Views

1. Create a new view in `src/views/`:

```vue
<template>
  <div class="my-view">
    <h1>My View</h1>
    <!-- Content -->
  </div>
</template>

<script setup lang="ts">
// Your code here
</script>

<style scoped>
.my-view {
  /* Styles */
}
</style>
```

2. Add route in `src/router/index.ts`:

```typescript
{
  path: 'my-view',
  name: 'MyView',
  component: () => import('@/views/MyViewView.vue')
}
```

3. Add menu item in `LayoutView.vue`:

```typescript
{
  label: 'My View',
  key: 'MyView',
  icon: () => h(NIcon, null, { default: () => h(MyIcon) })
}
```

## Authentication

The admin API uses JWT authentication. The token is stored in `localStorage` and automatically included in all API requests.

Login flow:
1. User enters credentials
2. Token received from `/api/auth/login`
3. Token stored in `localStorage`
4. Token included in `Authorization: Bearer <token>` header

## Deployment

### Production Build

```bash
pnpm build
```

Serve the `dist/` directory with your preferred web server.

### Nginx Configuration

```nginx
server {
    listen 80;
    server_name admin.dmpool.org;

    root /var/www/dmpool-admin/dist;
    index index.html;

    location / {
        try_files $uri $uri/ /index.html;
    }

    location /api {
        proxy_pass http://localhost:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

### Using Docker

```dockerfile
FROM node:18-alpine as builder
WORKDIR /app
COPY package*.json ./
RUN npm install
COPY . .
RUN npm run build

FROM nginx:alpine
COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 80
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `VITE_API_BASE_URL` | API base URL | `http://localhost:8080` |
| `VITE_API_TIMEOUT` | API timeout (ms) | `30000` |
| `VITE_APP_TITLE` | Application title | `DMPool Admin` |
| `VITE_APP_VERSION` | Application version | `2.4.0` |

## License

AGPLv3 - See LICENSE for details.
