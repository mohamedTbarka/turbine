# Turbine Dashboard Frontend Proposal

## Overview

This document outlines the proposal for building the Turbine web dashboard frontend. The backend REST API is already complete and production-ready.

## Current Status

### ✅ Backend (Complete)
- REST API with 16 endpoints
- Server-Sent Events (SSE) for real-time updates
- CORS support
- Running on Axum (Rust web framework)
- Full CRUD operations for tasks, queues, workers, and DLQ

### ⏳ Frontend (Pending)
- No UI implementation yet
- Backend API is ready and documented
- Needs modern web framework integration

## Proposed Technology Stack

### Option 1: React + TypeScript (Recommended)
**Pros:**
- Large ecosystem and community
- Excellent TypeScript support
- Many UI component libraries (shadcn/ui, MUI, Ant Design)
- Good SSE support via EventSource API

**Stack:**
- React 18+
- TypeScript
- Vite (build tool)
- TanStack Query (data fetching)
- Recharts or Chart.js (visualizations)
- Tailwind CSS + shadcn/ui (styling)
- Zustand or Jotai (state management)

### Option 2: Vue 3 + TypeScript
**Pros:**
- Simpler learning curve
- Built-in reactivity system
- Good TypeScript support
- Composition API is elegant

**Stack:**
- Vue 3 + Composition API
- TypeScript
- Vite
- Pinia (state management)
- Vuetify or Naive UI (components)
- Chart.js (visualizations)

### Option 3: Svelte + TypeScript
**Pros:**
- Smallest bundle size
- True reactivity without virtual DOM
- Easiest to learn
- Great developer experience

**Stack:**
- SvelteKit
- TypeScript
- Chart.js
- Carbon Components (IBM's design system)

## Dashboard Features

### 1. Overview Page
**Metrics:**
- Total tasks processed (24h, 7d, 30d)
- Tasks in progress
- Queue depths
- Active workers
- Success/failure rates
- Average task duration

**Visualizations:**
- Task throughput over time (line chart)
- Task state distribution (pie chart)
- Queue activity heatmap
- Worker utilization

### 2. Tasks View
**Features:**
- Real-time task list with SSE updates
- Filter by: state, queue, time range
- Search by task ID or name
- Task detail modal with:
  - Arguments
  - Result
  - Error/traceback
  - Execution timeline
- Bulk actions: revoke, retry

### 3. Queues View
**Features:**
- List all queues with stats
- Per-queue metrics:
  - Pending tasks
  - Processing tasks
  - Consumer count
  - Throughput
- Queue operations:
  - Purge queue
  - Pause/resume
  - Priority adjustment
- Queue depth over time chart

### 4. Workers View
**Features:**
- Active workers list
- Per-worker stats:
  - Tasks processed
  - Current task
  - Uptime
  - Memory usage
  - Status (idle, busy, offline)
- Worker health indicators
- Kill/restart operations

### 5. Dead Letter Queue (DLQ)
**Features:**
- Failed tasks list
- Filter by queue and time
- Task inspection with full context
- Bulk operations:
  - Reprocess
  - Delete
  - Export
- DLQ statistics dashboard

### 6. Metrics & Monitoring
**Features:**
- Live metrics view
- Prometheus metrics endpoint
- Custom metric queries
- Export to JSON/CSV
- Alert configuration (future)

## API Integration

### REST API Client

```typescript
// api/client.ts
import axios from 'axios';

const client = axios.create({
  baseURL: 'http://localhost:8080/api',
  timeout: 10000,
});

export const api = {
  // Overview
  getOverview: () => client.get('/overview'),

  // Workers
  getWorkers: () => client.get('/workers'),
  getWorker: (id: string) => client.get(`/workers/${id}`),

  // Queues
  getQueues: () => client.get('/queues'),
  getQueue: (name: string) => client.get(`/queues/${name}`),
  getQueueStats: (name: string) => client.get(`/queues/${name}/stats`),
  purgeQueue: (name: string) => client.post(`/queues/${name}/purge`),

  // Tasks
  getTasks: (params?: any) => client.get('/tasks', { params }),
  getTask: (id: string) => client.get(`/tasks/${id}`),
  revokeTask: (id: string) => client.post(`/tasks/${id}/revoke`),

  // DLQ
  getDLQ: (queue: string) => client.get(`/dlq/${queue}`),
  reprocessDLQ: (queue: string) => client.post(`/dlq/${queue}/reprocess`),
  purgeDLQ: (queue: string) => client.post(`/dlq/${queue}/purge`),

  // Metrics
  getMetrics: () => client.get('/metrics'),
};
```

### Server-Sent Events (SSE)

```typescript
// hooks/useSSE.ts
import { useEffect, useState } from 'react';

interface SSEEvent {
  type: 'task_started' | 'task_completed' | 'task_failed' | 'worker_connected';
  data: any;
}

export function useSSE(onEvent: (event: SSEEvent) => void) {
  useEffect(() => {
    const eventSource = new EventSource('http://localhost:8080/api/events');

    eventSource.onmessage = (event) => {
      const data = JSON.parse(event.data);
      onEvent(data);
    };

    eventSource.onerror = () => {
      console.error('SSE connection error');
    };

    return () => eventSource.close();
  }, [onEvent]);
}
```

## UI/UX Design

### Color Scheme
- Primary: Blue (#3B82F6)
- Success: Green (#10B981)
- Warning: Yellow (#F59E0B)
- Error: Red (#EF4444)
- Neutral: Gray (#6B7280)

### Layout
```
┌─────────────────────────────────────────────────┐
│  Turbine Dashboard                    [Profile] │
├──────────┬──────────────────────────────────────┤
│          │                                      │
│ Overview │   Main Content Area                  │
│ Tasks    │   - Charts                          │
│ Queues   │   - Tables                          │
│ Workers  │   - Real-time updates               │
│ DLQ      │                                      │
│ Metrics  │                                      │
│          │                                      │
└──────────┴──────────────────────────────────────┘
```

### Key Components

1. **Sidebar Navigation**
   - Collapsible
   - Active state indication
   - Icons + labels

2. **Data Table**
   - Sortable columns
   - Pagination
   - Row selection
   - Bulk actions

3. **Stat Cards**
   - Value + trend indicator
   - Sparkline chart
   - Color-coded status

4. **Real-time Chart**
   - Auto-updating via SSE
   - Configurable time range
   - Multiple series support

5. **Task Detail Modal**
   - JSON viewer for args/kwargs
   - Syntax highlighted traceback
   - Timeline visualization
   - Action buttons

## Development Setup

### Project Structure
```
dashboard/
├── src/
│   ├── api/
│   │   └── client.ts
│   ├── components/
│   │   ├── common/
│   │   ├── tasks/
│   │   ├── queues/
│   │   ├── workers/
│   │   └── dlq/
│   ├── hooks/
│   │   ├── useSSE.ts
│   │   └── useQuery.ts
│   ├── pages/
│   │   ├── Overview.tsx
│   │   ├── Tasks.tsx
│   │   ├── Queues.tsx
│   │   ├── Workers.tsx
│   │   ├── DLQ.tsx
│   │   └── Metrics.tsx
│   ├── store/
│   │   └── dashboard.ts
│   ├── types/
│   │   └── api.ts
│   ├── utils/
│   │   └── formatters.ts
│   ├── App.tsx
│   └── main.tsx
├── public/
├── index.html
├── package.json
├── tsconfig.json
└── vite.config.ts
```

### Getting Started

```bash
# Create new React + TypeScript project
npm create vite@latest dashboard -- --template react-ts
cd dashboard

# Install dependencies
npm install axios @tanstack/react-query recharts zustand
npm install -D @types/node tailwindcss autoprefixer postcss

# Setup Tailwind
npx tailwindcss init -p

# Start development server
npm run dev

# Build for production
npm run build
```

## Deployment

### Option 1: Separate Deployment
- Dashboard frontend: Vercel/Netlify
- Dashboard backend: Separate server
- CORS enabled

### Option 2: Embedded (Recommended)
- Compile React to static assets
- Embed in Rust binary using `include_dir!`
- Serve from `/` route
- API routes under `/api/*`

```rust
// Rust code to serve embedded frontend
use axum::Router;
use tower_http::services::ServeDir;

let app = Router::new()
    .nest("/api", api_routes())
    .nest_service("/", ServeDir::new("dashboard/dist"));
```

## Timeline Estimate

| Task | Estimated Time |
|------|----------------|
| Project setup + basic layout | 1-2 days |
| Overview page | 2-3 days |
| Tasks view | 3-4 days |
| Queues view | 2-3 days |
| Workers view | 2-3 days |
| DLQ view | 2-3 days |
| Metrics page | 2-3 days |
| SSE integration | 1-2 days |
| Polish + responsive design | 2-3 days |
| Testing + bug fixes | 2-3 days |
| **Total** | **3-4 weeks** |

## Next Steps

1. ✅ Document backend API (done)
2. ⏳ Choose frontend framework (community input needed)
3. ⏳ Create initial prototype
4. ⏳ Implement core features
5. ⏳ Add real-time updates
6. ⏳ Testing and polish
7. ⏳ Deployment strategy

## Contributing

Interested in building the dashboard? Check out:
- [GitHub Issues](https://github.com/turbine-queue/turbine/issues)
- Tag: `dashboard`, `frontend`, `good first issue`

We welcome contributions in any of the proposed frameworks!
