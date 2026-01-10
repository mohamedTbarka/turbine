# Turbine Dashboard Deployment Guide

## Option 1: Standalone Deployment

Deploy dashboard separately from Turbine server.

### Vercel

```bash
# Install Vercel CLI
npm i -g vercel

# Deploy
cd dashboard
vercel

# Production
vercel --prod
```

### Netlify

```bash
# Install Netlify CLI
npm i -g netlify-cli

# Deploy
cd dashboard
npm run build
netlify deploy --dir=build --prod
```

### Nginx

```bash
# Build
npm run build

# Copy to web root
sudo cp -r build/* /var/www/turbine-dashboard/

# Nginx config
server {
    listen 80;
    server_name dashboard.turbine.example.com;

    root /var/www/turbine-dashboard;
    index index.html;

    location / {
        try_files $uri $uri/ /index.html;
    }

    location /api {
        proxy_pass http://localhost:8080/api;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_cache_bypass $http_upgrade;
    }
}
```

## Option 2: Embedded in Rust Server

Serve dashboard from Turbine server (single binary).

### Step 1: Build Dashboard

```bash
cd dashboard
npm run build
```

### Step 2: Embed in Rust

Update `crates/turbine-dashboard/src/api.rs`:

```rust
use axum::Router;
use tower_http::services::ServeDir;

impl DashboardApi {
    pub fn router(&self) -> Router {
        let state = self.state.clone();

        Router::new()
            // API routes
            .nest("/api", self.api_routes())

            // Serve static dashboard files
            .nest_service("/", ServeDir::new("dashboard/build"))

            .with_state(state)
            .layer(CorsLayer::permissive())
            .layer(TraceLayer::new_for_http())
    }
}
```

### Step 3: Copy Built Files

```bash
# Copy built dashboard to Rust project
cp -r dashboard/build crates/turbine-dashboard/static

# Or use include_dir! macro
```

### Step 4: Build Rust Server

```bash
cargo build --release -p turbine-dashboard
```

Now dashboard is served at `http://localhost:8080/`

## Option 3: Docker Deployment

### Dockerfile

```dockerfile
# Build stage
FROM node:20-alpine AS builder

WORKDIR /app
COPY dashboard/package*.json ./
RUN npm ci

COPY dashboard/ ./
RUN npm run build

# Production stage
FROM nginx:alpine

# Copy built files
COPY --from=builder /app/build /usr/share/nginx/html

# Nginx configuration
COPY dashboard/nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
```

### nginx.conf

```nginx
server {
    listen 80;
    server_name _;

    root /usr/share/nginx/html;
    index index.html;

    # SPA fallback
    location / {
        try_files $uri $uri/ /index.html;
    }

    # Proxy API requests to Turbine server
    location /api {
        proxy_pass http://turbine-server:8080/api;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_cache_bypass $http_upgrade;

        # SSE support
        proxy_buffering off;
        proxy_read_timeout 24h;
    }
}
```

### Docker Compose

```yaml
version: '3.8'

services:
  turbine-server:
    image: turbine/turbine-server:latest
    ports:
      - "50051:50051"
      - "8080:8080"
    environment:
      - TURBINE_BROKER_URL=redis://redis:6379
    depends_on:
      - redis

  dashboard:
    build:
      context: .
      dockerfile: dashboard/Dockerfile
    ports:
      - "3000:80"
    depends_on:
      - turbine-server

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
```

## Environment Configuration

### Development

```bash
# .env.local
VITE_API_URL=http://localhost:8080/api
```

### Staging

```bash
# .env.staging
VITE_API_URL=https://staging-api.turbine.example.com/api
```

### Production

```bash
# .env.production
VITE_API_URL=https://api.turbine.example.com/api
```

Build with environment:

```bash
# Development
npm run build

# Production with custom env
VITE_API_URL=https://api.prod.com npm run build
```

## Performance Optimization

### Build Optimizations

Already configured in `vite.config.ts`:

- Code splitting
- Tree shaking
- Minification
- Asset optimization

### CDN Deployment

```bash
# Build with base path for CDN
vite build --base=/dashboard/

# Upload to CDN
aws s3 sync build/ s3://my-cdn-bucket/dashboard/
```

### Caching Headers

```nginx
location ~* \.(js|css|png|jpg|jpeg|gif|ico|svg)$ {
    expires 1y;
    add_header Cache-Control "public, immutable";
}
```

## Monitoring Dashboard Performance

### Lighthouse Score

```bash
npm i -g lighthouse

# Run audit
lighthouse http://localhost:3000 --view
```

**Target scores:**
- Performance: >90
- Accessibility: >95
- Best Practices: >90
- SEO: >90

### Bundle Size

Check bundle size:

```bash
npm run build

# Analyze
npx vite-bundle-visualizer
```

**Targets:**
- Initial JS: <100KB gzipped
- Initial CSS: <20KB gzipped
- Total page load: <200KB

## Security

### Content Security Policy

Add to nginx or meta tag:

```html
<meta
  http-equiv="Content-Security-Policy"
  content="default-src 'self'; connect-src 'self' http://localhost:8080; style-src 'self' 'unsafe-inline';"
/>
```

### HTTPS

Always use HTTPS in production:

```nginx
server {
    listen 443 ssl http2;
    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    # Strong SSL configuration
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;
}
```

## See Also

- [Dashboard API Reference](../docs/DASHBOARD_API.md)
- [Turbine Configuration](../docs/CONFIGURATION.md)
- [SvelteKit Documentation](https://kit.svelte.dev/docs)
