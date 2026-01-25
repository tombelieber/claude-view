# Claude View - Tech Stack Architecture

> Single-package architecture inspired by project-a/web

## Overview

```
claude-view/
├── package.json          # Single package for everything
├── tsconfig.json
├── vite.config.ts        # Frontend build + dev server proxy
├── tailwind.config.ts
├── index.html            # Frontend entry
├── src/
│   ├── main.tsx          # React entry
│   ├── App.tsx
│   ├── components/
│   ├── hooks/
│   ├── lib/
│   └── server/           # Express backend (separate build)
│       ├── index.ts      # CLI entry point
│       ├── server.ts
│       ├── sessions.ts
│       └── parser.ts
└── dist/
    ├── client/           # Vite build output
    └── server/           # tsc build output
```

## Core Dependencies

### From project-a/web (Foundation)

| Package | Version | Purpose |
|---------|---------|---------|
| react | ^19.2.0 | UI framework |
| react-dom | ^19.2.0 | React DOM renderer |
| vite | ^7.2.4 | Build tool + dev server |
| typescript | ~5.9.3 | Type safety |
| tailwindcss | ^4.1.18 | Styling |
| @tanstack/react-query | ^5.90.18 | Server state management |
| zustand | ^5.0.10 | Client state (sidebar, selection) |
| zod | ^4.3.5 | Runtime validation |
| clsx | ^2.1.1 | Conditional classes |
| tailwind-merge | ^3.4.0 | Merge Tailwind classes |
| lucide-react | ^0.562.0 | Icons |

### New for claude-view

| Package | Purpose |
|---------|---------|
| express | HTTP server for API + static files |
| get-port | Find available port |
| open | Auto-open browser |
| react-markdown | Render markdown in conversations |
| remark-gfm | GitHub-flavored markdown |
| prism-react-renderer | Syntax highlighting |

## package.json

```json
{
  "name": "claude-view",
  "version": "1.0.0",
  "description": "Browse and export Claude Code sessions as beautiful HTML",
  "type": "module",
  "bin": {
    "claude-view": "./dist/server/index.js"
  },
  "files": [
    "dist"
  ],
  "scripts": {
    "dev": "vite",
    "dev:server": "tsx watch src/server/index.ts",
    "build": "npm run build:client && npm run build:server",
    "build:client": "vite build",
    "build:server": "tsc -p tsconfig.server.json",
    "start": "node dist/server/index.js",
    "lint": "eslint .",
    "typecheck": "tsc --noEmit"
  },
  "dependencies": {
    "react": "^19.2.0",
    "react-dom": "^19.2.0",
    "@tanstack/react-query": "^5.90.18",
    "zustand": "^5.0.10",
    "zod": "^4.3.5",
    "clsx": "^2.1.1",
    "tailwind-merge": "^3.4.0",
    "lucide-react": "^0.562.0",
    "react-markdown": "^9.0.1",
    "remark-gfm": "^4.0.0",
    "prism-react-renderer": "^2.3.1",
    "express": "^4.21.0",
    "get-port": "^7.1.0",
    "open": "^10.1.0"
  },
  "devDependencies": {
    "@eslint/js": "^9.39.1",
    "@tailwindcss/vite": "^4.1.18",
    "@types/express": "^4.17.21",
    "@types/node": "^22.0.0",
    "@types/react": "^19.2.5",
    "@types/react-dom": "^19.2.3",
    "@vitejs/plugin-react": "^5.1.1",
    "autoprefixer": "^10.4.23",
    "eslint": "^9.39.1",
    "eslint-plugin-react-hooks": "^7.0.1",
    "eslint-plugin-react-refresh": "^0.4.24",
    "globals": "^17.0.0",
    "postcss": "^8.5.6",
    "tailwindcss": "^4.1.18",
    "tsx": "^4.19.0",
    "typescript": "~5.9.3",
    "typescript-eslint": "^8.46.4",
    "vite": "^7.2.4"
  }
}
```

## TypeScript Configuration

### tsconfig.json (Frontend)

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "noEmit": true,
    "skipLibCheck": true,
    "esModuleInterop": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "baseUrl": ".",
    "paths": {
      "@/*": ["./src/*"]
    }
  },
  "include": ["src/**/*"],
  "exclude": ["src/server/**/*"]
}
```

### tsconfig.server.json (Backend)

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "node",
    "outDir": "./dist/server",
    "rootDir": "./src/server",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "declaration": true
  },
  "include": ["src/server/**/*"]
}
```

## Vite Configuration

```typescript
// vite.config.ts
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    proxy: {
      '/api': 'http://localhost:3000',
    },
  },
  build: {
    outDir: 'dist/client',
  },
})
```

## Project Structure

```
src/
├── main.tsx                    # React entry point
├── App.tsx                     # Root component with routing
├── index.css                   # Tailwind imports
│
├── components/
│   ├── ui/                     # Reusable UI primitives
│   │   ├── button.tsx
│   │   ├── card.tsx
│   │   ├── collapsible.tsx
│   │   └── scroll-area.tsx
│   │
│   ├── layout/
│   │   ├── sidebar.tsx         # Project tree sidebar
│   │   ├── header.tsx          # Top navigation bar
│   │   └── layout.tsx          # Main layout wrapper
│   │
│   ├── session/
│   │   ├── session-list.tsx    # List of sessions for a project
│   │   ├── session-card.tsx    # Individual session preview
│   │   └── session-tree.tsx    # Project/session tree view
│   │
│   ├── conversation/
│   │   ├── conversation-view.tsx
│   │   ├── message.tsx         # Single message component
│   │   ├── tool-badge.tsx      # Collapsible tool call summary
│   │   └── code-block.tsx      # Syntax-highlighted code
│   │
│   └── export/
│       ├── export-modal.tsx    # Export options dialog
│       └── export-preview.tsx  # HTML preview
│
├── hooks/
│   ├── use-projects.ts         # React Query: fetch projects
│   ├── use-session.ts          # React Query: fetch session content
│   └── use-export.ts           # HTML generation logic
│
├── lib/
│   ├── api.ts                  # API client functions
│   ├── utils.ts                # cn(), formatDate(), etc.
│   ├── export-html.ts          # Standalone HTML generator
│   └── markdown.ts             # Markdown processing helpers
│
├── stores/
│   └── app-store.ts            # Zustand: sidebar state, selection
│
└── server/
    ├── index.ts                # CLI entry: parse args, start server
    ├── server.ts               # Express app setup
    ├── sessions.ts             # Discover projects/sessions
    └── parser.ts               # Parse JSONL conversation files
```

## Data Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                         User's Machine                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ~/.claude/projects/                                            │
│  └── -Users-name-project/                                       │
│      └── abc123.jsonl          ◄─── JSONL session files         │
│                                                                 │
│         │                                                       │
│         ▼                                                       │
│  ┌─────────────────────┐                                        │
│  │   Express Server    │                                        │
│  │   (localhost:3000)  │                                        │
│  │                     │                                        │
│  │  GET /api/projects  │────► List all projects + sessions      │
│  │  GET /api/session/* │────► Parse and return messages         │
│  │  GET /*             │────► Serve React app (static)          │
│  └─────────────────────┘                                        │
│         ▲                                                       │
│         │ HTTP                                                  │
│         ▼                                                       │
│  ┌─────────────────────┐                                        │
│  │   React Frontend    │                                        │
│  │                     │                                        │
│  │  useProjects()      │────► React Query fetches /api/projects │
│  │  useSession(id)     │────► React Query fetches /api/session  │
│  │  appStore           │────► Zustand manages UI state          │
│  │                     │                                        │
│  │  exportHtml()       │────► Client-side HTML generation       │
│  └─────────────────────┘                                        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## API Endpoints

| Method | Path | Response |
|--------|------|----------|
| GET | `/api/projects` | `ProjectInfo[]` - All projects with session metadata |
| GET | `/api/session/:projectDir/:sessionId` | `ParsedSession` - Messages + metadata |
| GET | `/*` | Static files (React app) |

## State Management

### React Query (Server State)

```typescript
// hooks/use-projects.ts
export function useProjects() {
  return useQuery({
    queryKey: ['projects'],
    queryFn: () => api.getProjects(),
    staleTime: 30_000, // Refetch after 30s
  })
}

// hooks/use-session.ts
export function useSession(projectDir: string, sessionId: string) {
  return useQuery({
    queryKey: ['session', projectDir, sessionId],
    queryFn: () => api.getSession(projectDir, sessionId),
    enabled: !!sessionId,
  })
}
```

### Zustand (Client State)

```typescript
// stores/app-store.ts
interface AppState {
  sidebarOpen: boolean
  selectedProject: string | null
  selectedSession: { projectDir: string; sessionId: string } | null
  expandedProjects: Set<string>

  toggleSidebar: () => void
  selectProject: (name: string) => void
  selectSession: (projectDir: string, sessionId: string) => void
  toggleProjectExpanded: (name: string) => void
}

export const useAppStore = create<AppState>((set) => ({
  sidebarOpen: true,
  selectedProject: null,
  selectedSession: null,
  expandedProjects: new Set(),

  toggleSidebar: () => set((s) => ({ sidebarOpen: !s.sidebarOpen })),
  selectProject: (name) => set({ selectedProject: name }),
  selectSession: (projectDir, sessionId) =>
    set({ selectedSession: { projectDir, sessionId } }),
  toggleProjectExpanded: (name) => set((s) => {
    const next = new Set(s.expandedProjects)
    next.has(name) ? next.delete(name) : next.add(name)
    return { expandedProjects: next }
  }),
}))
```

## Development Workflow

```bash
# Terminal 1: Run backend
bun run dev:server

# Terminal 2: Run frontend (proxies API to backend)
bun run dev

# Or use a single command with concurrently (optional)
bun x concurrently "bun run dev:server" "bun run dev"
```

## Production Build

```bash
# Build everything
bun run build

# Output:
# dist/client/     <- Vite build (static files)
# dist/server/     <- tsc build (Node.js server)

# Run production
bun run start
# or
npx claude-view  # Works for end users (npm compatible)
```

## Key Differences from Original Plan

| Original Plan | This Architecture |
|---------------|-------------------|
| Separate `client/` folder with own package.json | Single package, `src/server/` for backend |
| Two npm installs needed | One `npm install` |
| Manual build coordination | Single `npm run build` |
| Basic React (no React Query) | React Query for data fetching |
| No state management lib | Zustand for UI state |
| Basic TypeScript | Path aliases, stricter config |

## Next Steps

1. Initialize package.json with dependencies
2. Set up Vite + TypeScript configs
3. Create server with session discovery
4. Build React components per UI mockups
5. Add export functionality
6. Test and publish to npm
