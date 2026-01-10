# Turbine Dashboard (Svelte)

Modern, real-time web dashboard for monitoring and managing Turbine task queues.

## Features

- 📊 **Real-time Overview** - Task throughput, success rates, queue depths
- 📋 **Task Management** - View, filter, search, and revoke tasks
- 📬 **Queue Monitoring** - Monitor queue depths and purge operations
- ⚙️ **Worker Tracking** - See active workers and their status
- ⚠️ **Dead Letter Queue** - Inspect and reprocess failed tasks
- 📈 **Metrics Dashboard** - View Prometheus metrics
- 🔄 **Live Updates** - Server-Sent Events for real-time data
- 🎨 **Modern UI** - Built with Svelte, TailwindCSS, and Chart.js

## Prerequisites

- Node.js 18+ and npm/pnpm
- Turbine server running on `localhost:8080` (or configure `VITE_API_URL`)

## Quick Start

### Development

```bash
cd dashboard

# Install dependencies
npm install

# Start dev server
npm run dev

# Open http://localhost:3000
```

### Production Build

```bash
# Build for production
npm run build

# Preview production build
npm run preview

# Deploy the 'build' directory
```

## Configuration

### API URL

Set the Turbine backend API URL:

```bash
# .env.local
VITE_API_URL=http://localhost:8080/api
```

Or use default (`http://localhost:8080/api`).

### Proxy Configuration

The dev server proxies `/api/*` requests to the backend (configured in `vite.config.ts`):

```typescript
server: {
  proxy: {
    '/api': {
      target: 'http://localhost:8080',
      changeOrigin: true
    }
  }
}
```

## Project Structure

```
dashboard/
├── src/
│   ├── lib/
│   │   ├── api/
│   │   │   └── client.ts          # API client
│   │   ├── components/
│   │   │   ├── StatCard.svelte
│   │   │   ├── ThroughputChart.svelte
│   │   │   ├── TaskStatesChart.svelte
│   │   │   ├── TaskRow.svelte
│   │   │   └── TaskModal.svelte
│   │   └── stores/
│   │       └── events.ts           # SSE store
│   ├── routes/
│   │   ├── +layout.svelte          # Main layout with nav
│   │   ├── +page.svelte            # Overview page
│   │   ├── tasks/
│   │   │   └── +page.svelte        # Tasks page
│   │   ├── queues/
│   │   │   └── +page.svelte        # Queues page
│   │   ├── workers/
│   │   │   └── +page.svelte        # Workers page
│   │   ├── dlq/
│   │   │   └── +page.svelte        # DLQ page
│   │   └── metrics/
│   │       └── +page.svelte        # Metrics page
│   ├── app.css                     # Tailwind styles
│   └── app.html                    # HTML template
├── package.json
├── svelte.config.js
├── vite.config.ts
├── tailwind.config.js
└── tsconfig.json
```

## Pages

### Overview (`/`)

- Task statistics (pending, running, success rate)
- Worker status
- Real-time throughput chart
- Task state distribution (pie chart)
- Queue summary

### Tasks (`/tasks`)

- List all tasks with pagination
- Filter by state (pending, running, success, failed)
- Filter by queue
- Search by task ID or name
- View task details (args, result, error, traceback)
- Revoke pending/running tasks
- Auto-refresh every 5 seconds

### Queues (`/queues`)

- List all queues
- Queue metrics (pending, processing, consumers, throughput)
- Purge queue operations
- Visual queue depth indicators
- Auto-refresh every 5 seconds

### Workers (`/workers`)

- List active workers
- Worker status (active, idle, offline)
- Tasks processed count
- Current task being processed
- Worker uptime
- Queues assigned to each worker

### DLQ (`/dlq`)

- List failed tasks after max retries
- View failure reasons and tracebacks
- Reprocess failed tasks
- Purge DLQ
- Filter by queue

### Metrics (`/metrics`)

- View raw Prometheus metrics
- Filter metrics by keyword
- Quick filter buttons
- Copy metrics endpoint data

## Real-Time Updates

The dashboard uses Server-Sent Events (SSE) for live updates:

- Task started/completed/failed events
- Worker connected/disconnected events
- Automatic reconnection on connection loss
- Connection status indicator in sidebar

## Customization

### Theme Colors

Edit `tailwind.config.js` to customize colors:

```javascript
theme: {
  extend: {
    colors: {
      primary: {
        // Your custom color palette
      }
    }
  }
}
```

### API Client

Extend the API client in `src/lib/api/client.ts` to add new endpoints.

### Adding Pages

Create new page in `src/routes/your-page/+page.svelte`:

```svelte
<script lang="ts">
  // Your page logic
</script>

<div class="p-8">
  <h1 class="text-3xl font-bold">Your Page</h1>
</div>
```

Update navigation in `src/routes/+layout.svelte`.

## Deployment

### Static Build

```bash
npm run build

# Deploy the 'build' directory to:
# - Netlify
# - Vercel
# - GitHub Pages
# - Nginx/Apache
```

### With Turbine Server

Serve dashboard from Turbine server (embedded):

1. Build dashboard: `npm run build`
2. Copy `build/` to `crates/turbine-dashboard/static/`
3. Configure Rust server to serve static files from `/`

### Docker

```dockerfile
FROM node:20-alpine AS builder

WORKDIR /app
COPY package*.json ./
RUN npm ci

COPY . .
RUN npm run build

FROM nginx:alpine
COPY --from=builder /app/build /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80
```

## Troubleshooting

### CORS Errors

Ensure Turbine server has CORS enabled:

```bash
./turbine-dashboard --cors
```

### Connection Refused

Check that Turbine server is running:

```bash
curl http://localhost:8080/api/health
```

### SSE Not Connecting

1. Check console for errors
2. Verify `/api/events` endpoint is accessible
3. Check browser compatibility (all modern browsers support SSE)

## Browser Support

- Chrome/Edge 90+
- Firefox 88+
- Safari 14+

## Contributing

Improvements welcome! See [CONTRIBUTING.md](../CONTRIBUTING.md).

## License

MIT/Apache-2.0 (same as Turbine)
