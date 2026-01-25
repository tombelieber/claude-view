# Claude View Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a one-command tool that lets users browse and export Claude Code sessions as beautiful, shareable HTML files.

**Architecture:** Node.js CLI that spawns a local web server, auto-opens browser, displays a session browser UI, and exports conversations to standalone HTML with Claude.ai-style design. Backend reads `~/.claude/projects/` JSONL files. Frontend renders markdown, collapses tool calls, and provides one-click export.

**Tech Stack:** Node.js, TypeScript, Express, React, Tailwind CSS, Vite, react-markdown, Prism.js (syntax highlighting)

---

## Design Summary

### User Experience
```bash
npx claude-view
```
1. Scans ports 3000-3100, picks first available
2. Starts local server
3. Auto-opens browser at `http://localhost:<port>`
4. User sees session browser → picks session → exports HTML

### Key Features
- **Session Browser:** Tree view of `~/.claude/projects/`, sorted by date
- **Session Preview:** Shows first few messages to identify session
- **Conversation View:** Claude.ai style cards, markdown rendered
- **Tool Activity:** Collapsible badges (e.g., "📁 Read 3 files")
- **Code Blocks:** Syntax highlighted, collapsed by default (first 5 lines)
- **Export:** Download as standalone HTML file

### Content Filtering
- Show: User messages, Assistant text responses
- Collapse: Tool calls, thinking blocks
- Hide: Internal metadata, system messages

---

## Task 1: Project Scaffolding

**Files:**
- Create: `package.json`
- Create: `tsconfig.json`
- Create: `src/cli.ts`
- Create: `src/server.ts`

**Step 1: Initialize npm package**

```bash
mkdir -p ~/dev/claude-view
cd ~/dev/claude-view
npm init -y
```

**Step 2: Install dependencies**

```bash
npm install express open get-port
npm install -D typescript @types/node @types/express ts-node
```

**Step 3: Create tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "ESNext",
    "moduleResolution": "node",
    "esModuleInterop": true,
    "strict": true,
    "outDir": "./dist",
    "rootDir": "./src",
    "declaration": true
  },
  "include": ["src/**/*"]
}
```

**Step 4: Create minimal CLI entry point**

Create `src/cli.ts`:
```typescript
#!/usr/bin/env node
import { startServer } from './server.js';

async function main() {
  const port = await startServer();
  console.log(`✓ Claude Session Viewer running at http://localhost:${port}`);
}

main().catch(console.error);
```

**Step 5: Create minimal server**

Create `src/server.ts`:
```typescript
import express from 'express';
import getPort from 'get-port';
import open from 'open';

export async function startServer(): Promise<number> {
  const app = express();
  const port = await getPort({ port: [3000, 3001, 3002, 3003, 3004, 3005] });

  app.get('/', (req, res) => {
    res.send('<h1>Claude Session Viewer</h1><p>Coming soon...</p>');
  });

  return new Promise((resolve) => {
    app.listen(port, () => {
      open(`http://localhost:${port}`);
      resolve(port);
    });
  });
}
```

**Step 6: Update package.json**

Add to package.json:
```json
{
  "type": "module",
  "bin": {
    "claude-view": "./dist/cli.js"
  },
  "scripts": {
    "build": "tsc",
    "start": "node dist/cli.js",
    "dev": "ts-node src/cli.ts"
  }
}
```

**Step 7: Build and test**

```bash
npm run build
npm start
```
Expected: Browser opens, shows "Claude Session Viewer - Coming soon..."

**Step 8: Commit**

```bash
git init
git add .
git commit -m "feat: initial project scaffolding with CLI and server"
```

---

## Task 2: Session Discovery API

**Files:**
- Create: `src/sessions.ts`
- Modify: `src/server.ts`

**Step 1: Create session discovery module**

Create `src/sessions.ts`:
```typescript
import { readdir, readFile, stat } from 'fs/promises';
import { join, basename } from 'path';
import { homedir } from 'os';

export interface SessionInfo {
  id: string;
  project: string;
  projectPath: string;
  filePath: string;
  modifiedAt: Date;
  sizeBytes: number;
  preview: string;
}

export interface ProjectInfo {
  name: string;
  path: string;
  sessions: SessionInfo[];
}

const CLAUDE_PROJECTS_DIR = join(homedir(), '.claude', 'projects');

export async function getProjects(): Promise<ProjectInfo[]> {
  const projects: ProjectInfo[] = [];

  try {
    const entries = await readdir(CLAUDE_PROJECTS_DIR);

    for (const entry of entries) {
      if (entry.startsWith('.')) continue;

      const projectPath = join(CLAUDE_PROJECTS_DIR, entry);
      const projectStat = await stat(projectPath);

      if (!projectStat.isDirectory()) continue;

      const sessions = await getSessionsForProject(entry, projectPath);
      if (sessions.length > 0) {
        projects.push({
          name: formatProjectName(entry),
          path: projectPath,
          sessions: sessions.sort((a, b) => b.modifiedAt.getTime() - a.modifiedAt.getTime())
        });
      }
    }
  } catch (error) {
    console.error('Error reading projects:', error);
  }

  return projects.sort((a, b) => {
    const aLatest = a.sessions[0]?.modifiedAt.getTime() || 0;
    const bLatest = b.sessions[0]?.modifiedAt.getTime() || 0;
    return bLatest - aLatest;
  });
}

async function getSessionsForProject(projectName: string, projectPath: string): Promise<SessionInfo[]> {
  const sessions: SessionInfo[] = [];
  const entries = await readdir(projectPath);

  for (const entry of entries) {
    if (!entry.endsWith('.jsonl')) continue;

    const filePath = join(projectPath, entry);
    const fileStat = await stat(filePath);
    const preview = await getSessionPreview(filePath);

    sessions.push({
      id: basename(entry, '.jsonl'),
      project: projectName,
      projectPath,
      filePath,
      modifiedAt: fileStat.mtime,
      sizeBytes: fileStat.size,
      preview
    });
  }

  return sessions;
}

async function getSessionPreview(filePath: string): Promise<string> {
  try {
    const content = await readFile(filePath, 'utf-8');
    const lines = content.split('\n').slice(0, 10);

    for (const line of lines) {
      if (!line.trim()) continue;
      try {
        const parsed = JSON.parse(line);
        if (parsed.type === 'user' && parsed.message?.content) {
          const content = typeof parsed.message.content === 'string'
            ? parsed.message.content
            : parsed.message.content[0]?.text || '';
          return content.slice(0, 100) + (content.length > 100 ? '...' : '');
        }
      } catch {}
    }
  } catch {}
  return 'No preview available';
}

function formatProjectName(dirName: string): string {
  return dirName
    .replace(/^-/, '')
    .replace(/-/g, '/')
    .replace(/--/g, '-');
}
```

**Step 2: Add API endpoint to server**

Modify `src/server.ts`:
```typescript
import express from 'express';
import getPort from 'get-port';
import open from 'open';
import { getProjects } from './sessions.js';

export async function startServer(): Promise<number> {
  const app = express();
  const port = await getPort({ port: [3000, 3001, 3002, 3003, 3004, 3005] });

  app.get('/api/projects', async (req, res) => {
    const projects = await getProjects();
    res.json(projects);
  });

  app.get('/', (req, res) => {
    res.send('<h1>Claude Session Viewer</h1><p>Coming soon...</p>');
  });

  return new Promise((resolve) => {
    app.listen(port, () => {
      open(`http://localhost:${port}`);
      resolve(port);
    });
  });
}
```

**Step 3: Test the API**

```bash
npm run build && npm start
# In another terminal:
curl http://localhost:3000/api/projects | jq '.[0]'
```
Expected: JSON with project name, sessions array

**Step 4: Commit**

```bash
git add .
git commit -m "feat: add session discovery API"
```

---

## Task 3: Session Content Parser

**Files:**
- Create: `src/parser.ts`
- Modify: `src/server.ts`

**Step 1: Create JSONL parser**

Create `src/parser.ts`:
```typescript
import { readFile } from 'fs/promises';

export interface ToolCall {
  name: string;
  count: number;
}

export interface Message {
  role: 'user' | 'assistant';
  content: string;
  timestamp?: string;
  toolCalls?: ToolCall[];
}

export interface ParsedSession {
  messages: Message[];
  metadata: {
    totalMessages: number;
    toolCallCount: number;
  };
}

export async function parseSession(filePath: string): Promise<ParsedSession> {
  const content = await readFile(filePath, 'utf-8');
  const lines = content.split('\n').filter(l => l.trim());

  const messages: Message[] = [];
  const toolCallMap = new Map<string, number>();
  let pendingToolCalls: ToolCall[] = [];

  for (const line of lines) {
    try {
      const entry = JSON.parse(line);

      if (entry.type === 'user' && entry.message?.content) {
        // Attach pending tool calls to previous assistant message
        if (pendingToolCalls.length > 0 && messages.length > 0) {
          const lastMsg = messages[messages.length - 1];
          if (lastMsg.role === 'assistant') {
            lastMsg.toolCalls = [...pendingToolCalls];
          }
        }
        pendingToolCalls = [];

        messages.push({
          role: 'user',
          content: extractContent(entry.message.content),
          timestamp: entry.timestamp
        });
      }

      if (entry.type === 'assistant' && entry.message?.content) {
        const textContent = extractAssistantText(entry.message.content);
        if (textContent) {
          messages.push({
            role: 'assistant',
            content: textContent,
            timestamp: entry.timestamp
          });
        }

        // Track tool uses in this message
        const tools = extractToolUses(entry.message.content);
        for (const tool of tools) {
          toolCallMap.set(tool, (toolCallMap.get(tool) || 0) + 1);
          const existing = pendingToolCalls.find(t => t.name === tool);
          if (existing) {
            existing.count++;
          } else {
            pendingToolCalls.push({ name: tool, count: 1 });
          }
        }
      }
    } catch {}
  }

  // Attach any remaining tool calls
  if (pendingToolCalls.length > 0 && messages.length > 0) {
    const lastMsg = messages[messages.length - 1];
    if (lastMsg.role === 'assistant') {
      lastMsg.toolCalls = [...pendingToolCalls];
    }
  }

  return {
    messages,
    metadata: {
      totalMessages: messages.length,
      toolCallCount: Array.from(toolCallMap.values()).reduce((a, b) => a + b, 0)
    }
  };
}

function extractContent(content: unknown): string {
  if (typeof content === 'string') return content;
  if (Array.isArray(content)) {
    return content
      .filter(block => block.type === 'text')
      .map(block => block.text)
      .join('\n');
  }
  return '';
}

function extractAssistantText(content: unknown): string {
  if (typeof content === 'string') return content;
  if (Array.isArray(content)) {
    return content
      .filter(block => block.type === 'text')
      .map(block => block.text)
      .join('\n');
  }
  return '';
}

function extractToolUses(content: unknown): string[] {
  if (!Array.isArray(content)) return [];
  return content
    .filter(block => block.type === 'tool_use')
    .map(block => block.name);
}
```

**Step 2: Add session content endpoint**

Add to `src/server.ts`:
```typescript
import { parseSession } from './parser.js';
import { homedir } from 'os';
import { join } from 'path';

// Add this route in startServer():
app.get('/api/session/:projectDir/:sessionId', async (req, res) => {
  const { projectDir, sessionId } = req.params;
  const filePath = join(homedir(), '.claude', 'projects', projectDir, `${sessionId}.jsonl`);

  try {
    const session = await parseSession(filePath);
    res.json(session);
  } catch (error) {
    res.status(404).json({ error: 'Session not found' });
  }
});
```

**Step 3: Test**

```bash
npm run build && npm start
# Get a session ID from /api/projects, then:
curl http://localhost:3000/api/session/-Users-TBGor/<session-id> | jq '.messages | length'
```

**Step 4: Commit**

```bash
git add .
git commit -m "feat: add session content parser with tool call grouping"
```

---

## Task 4: React Frontend Setup

**Files:**
- Create: `client/package.json`
- Create: `client/index.html`
- Create: `client/src/App.tsx`
- Create: `client/src/main.tsx`
- Create: `client/vite.config.ts`
- Create: `client/tailwind.config.js`

**Step 1: Initialize client package**

```bash
mkdir -p client
cd client
npm init -y
npm install react react-dom react-markdown remark-gfm
npm install -D vite @vitejs/plugin-react typescript @types/react @types/react-dom tailwindcss postcss autoprefixer
npx tailwindcss init -p
```

**Step 2: Create vite.config.ts**

```typescript
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      '/api': 'http://localhost:3000'
    }
  },
  build: {
    outDir: '../dist/client'
  }
});
```

**Step 3: Create tailwind.config.js**

```javascript
/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {},
  },
  plugins: [],
};
```

**Step 4: Create index.html**

```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Claude Session Viewer</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

**Step 5: Create src/main.tsx**

```tsx
import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './index.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
```

**Step 6: Create src/index.css**

```css
@tailwind base;
@tailwind components;
@tailwind utilities;

body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
}
```

**Step 7: Create src/App.tsx**

```tsx
import { useState, useEffect } from 'react';

interface SessionInfo {
  id: string;
  project: string;
  modifiedAt: string;
  preview: string;
}

interface ProjectInfo {
  name: string;
  sessions: SessionInfo[];
}

export default function App() {
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetch('/api/projects')
      .then(res => res.json())
      .then(data => {
        setProjects(data);
        setLoading(false);
      });
  }, []);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-screen">
        <p className="text-gray-500">Loading sessions...</p>
      </div>
    );
  }

  return (
    <div className="max-w-4xl mx-auto p-6">
      <h1 className="text-2xl font-semibold mb-6">Claude Session Viewer</h1>
      {projects.map(project => (
        <div key={project.name} className="mb-6">
          <h2 className="text-lg font-medium text-gray-700 mb-2">{project.name}</h2>
          <div className="space-y-2">
            {project.sessions.slice(0, 5).map(session => (
              <div
                key={session.id}
                className="p-3 border rounded-lg hover:bg-gray-50 cursor-pointer"
              >
                <p className="text-sm text-gray-600 truncate">{session.preview}</p>
                <p className="text-xs text-gray-400 mt-1">
                  {new Date(session.modifiedAt).toLocaleString()}
                </p>
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
```

**Step 8: Test frontend**

```bash
cd client
npm run dev
```
Expected: Browser shows session list with previews

**Step 9: Commit**

```bash
cd ..
git add .
git commit -m "feat: add React frontend with session browser"
```

---

## Task 5: Conversation View Component

**Files:**
- Create: `client/src/components/ConversationView.tsx`
- Create: `client/src/components/Message.tsx`
- Create: `client/src/components/ToolBadge.tsx`
- Create: `client/src/components/CodeBlock.tsx`
- Modify: `client/src/App.tsx`

**Step 1: Create ToolBadge component**

Create `client/src/components/ToolBadge.tsx`:
```tsx
import { useState } from 'react';

interface ToolCall {
  name: string;
  count: number;
}

interface ToolBadgeProps {
  tools: ToolCall[];
}

const TOOL_ICONS: Record<string, string> = {
  Read: '📄',
  Write: '✏️',
  Edit: '🔧',
  Bash: '💻',
  Glob: '🔍',
  Grep: '🔎',
  default: '🔧'
};

export default function ToolBadge({ tools }: ToolBadgeProps) {
  const [expanded, setExpanded] = useState(false);

  const totalCount = tools.reduce((sum, t) => sum + t.count, 0);
  const summary = tools.slice(0, 2).map(t =>
    `${TOOL_ICONS[t.name] || TOOL_ICONS.default} ${t.name}`
  ).join(', ');

  return (
    <div className="my-2">
      <button
        onClick={() => setExpanded(!expanded)}
        className="text-xs text-gray-500 bg-gray-100 px-2 py-1 rounded-full hover:bg-gray-200"
      >
        {expanded ? '▼' : '▶'} {summary}{tools.length > 2 ? '...' : ''} ({totalCount} calls)
      </button>
      {expanded && (
        <div className="mt-2 pl-4 text-xs text-gray-500 space-y-1">
          {tools.map((tool, i) => (
            <div key={i}>
              {TOOL_ICONS[tool.name] || TOOL_ICONS.default} {tool.name} × {tool.count}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
```

**Step 2: Create CodeBlock component**

Create `client/src/components/CodeBlock.tsx`:
```tsx
import { useState } from 'react';

interface CodeBlockProps {
  language?: string;
  children: string;
}

export default function CodeBlock({ language, children }: CodeBlockProps) {
  const [expanded, setExpanded] = useState(false);
  const lines = children.split('\n');
  const shouldCollapse = lines.length > 5;
  const displayLines = expanded || !shouldCollapse ? lines : lines.slice(0, 5);

  return (
    <div className="my-3 rounded-lg overflow-hidden border border-gray-200">
      {language && (
        <div className="bg-gray-100 px-3 py-1 text-xs text-gray-600 flex justify-between">
          <span>{language}</span>
          <button
            onClick={() => navigator.clipboard.writeText(children)}
            className="hover:text-gray-900"
          >
            Copy
          </button>
        </div>
      )}
      <pre className="p-3 bg-gray-50 overflow-x-auto text-sm">
        <code>{displayLines.join('\n')}</code>
      </pre>
      {shouldCollapse && (
        <button
          onClick={() => setExpanded(!expanded)}
          className="w-full py-1 text-xs text-gray-500 bg-gray-100 hover:bg-gray-200"
        >
          {expanded ? 'Collapse' : `Show ${lines.length - 5} more lines`}
        </button>
      )}
    </div>
  );
}
```

**Step 3: Create Message component**

Create `client/src/components/Message.tsx`:
```tsx
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import CodeBlock from './CodeBlock';
import ToolBadge from './ToolBadge';

interface ToolCall {
  name: string;
  count: number;
}

interface MessageProps {
  role: 'user' | 'assistant';
  content: string;
  toolCalls?: ToolCall[];
}

export default function Message({ role, content, toolCalls }: MessageProps) {
  const isUser = role === 'user';

  return (
    <div className={`py-6 ${isUser ? 'bg-white' : 'bg-gray-50'}`}>
      <div className="max-w-3xl mx-auto px-4">
        <div className="flex gap-4">
          <div className={`w-8 h-8 rounded-full flex items-center justify-center text-white text-sm ${
            isUser ? 'bg-blue-500' : 'bg-orange-500'
          }`}>
            {isUser ? 'U' : 'C'}
          </div>
          <div className="flex-1 min-w-0">
            <p className="text-sm font-medium text-gray-700 mb-2">
              {isUser ? 'You' : 'Claude'}
            </p>
            {isUser ? (
              <p className="text-gray-800 whitespace-pre-wrap">{content}</p>
            ) : (
              <div className="prose prose-sm max-w-none">
                <ReactMarkdown
                  remarkPlugins={[remarkGfm]}
                  components={{
                    code({ node, inline, className, children, ...props }: any) {
                      const match = /language-(\w+)/.exec(className || '');
                      if (!inline && match) {
                        return (
                          <CodeBlock language={match[1]}>
                            {String(children).replace(/\n$/, '')}
                          </CodeBlock>
                        );
                      }
                      return (
                        <code className="bg-gray-100 px-1 rounded" {...props}>
                          {children}
                        </code>
                      );
                    }
                  }}
                >
                  {content}
                </ReactMarkdown>
              </div>
            )}
            {toolCalls && toolCalls.length > 0 && (
              <ToolBadge tools={toolCalls} />
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
```

**Step 4: Create ConversationView component**

Create `client/src/components/ConversationView.tsx`:
```tsx
import { useState, useEffect } from 'react';
import Message from './Message';

interface ToolCall {
  name: string;
  count: number;
}

interface MessageData {
  role: 'user' | 'assistant';
  content: string;
  toolCalls?: ToolCall[];
}

interface ConversationViewProps {
  projectDir: string;
  sessionId: string;
  onBack: () => void;
}

export default function ConversationView({ projectDir, sessionId, onBack }: ConversationViewProps) {
  const [messages, setMessages] = useState<MessageData[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetch(`/api/session/${projectDir}/${sessionId}`)
      .then(res => res.json())
      .then(data => {
        setMessages(data.messages);
        setLoading(false);
      });
  }, [projectDir, sessionId]);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-screen">
        <p className="text-gray-500">Loading conversation...</p>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-white">
      <header className="sticky top-0 bg-white border-b px-4 py-3 flex items-center gap-4">
        <button onClick={onBack} className="text-gray-600 hover:text-gray-900">
          ← Back
        </button>
        <h1 className="font-medium">Conversation</h1>
        <button
          onClick={() => {/* TODO: export */}}
          className="ml-auto px-3 py-1 bg-blue-500 text-white text-sm rounded hover:bg-blue-600"
        >
          Export HTML
        </button>
      </header>
      <div className="divide-y">
        {messages.map((msg, i) => (
          <Message key={i} {...msg} />
        ))}
      </div>
    </div>
  );
}
```

**Step 5: Update App.tsx to use components**

Replace `client/src/App.tsx`:
```tsx
import { useState, useEffect } from 'react';
import ConversationView from './components/ConversationView';

interface SessionInfo {
  id: string;
  project: string;
  modifiedAt: string;
  preview: string;
}

interface ProjectInfo {
  name: string;
  path: string;
  sessions: SessionInfo[];
}

export default function App() {
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedSession, setSelectedSession] = useState<{
    projectDir: string;
    sessionId: string;
  } | null>(null);

  useEffect(() => {
    fetch('/api/projects')
      .then(res => res.json())
      .then(data => {
        setProjects(data);
        setLoading(false);
      });
  }, []);

  if (selectedSession) {
    return (
      <ConversationView
        projectDir={selectedSession.projectDir}
        sessionId={selectedSession.sessionId}
        onBack={() => setSelectedSession(null)}
      />
    );
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center h-screen">
        <p className="text-gray-500">Loading sessions...</p>
      </div>
    );
  }

  return (
    <div className="max-w-4xl mx-auto p-6">
      <h1 className="text-2xl font-semibold mb-6">Claude Session Viewer</h1>
      {projects.map(project => {
        const projectDir = project.path.split('/').pop() || '';
        return (
          <div key={project.name} className="mb-6">
            <h2 className="text-lg font-medium text-gray-700 mb-2">{project.name}</h2>
            <div className="space-y-2">
              {project.sessions.slice(0, 5).map(session => (
                <div
                  key={session.id}
                  onClick={() => setSelectedSession({ projectDir, sessionId: session.id })}
                  className="p-3 border rounded-lg hover:bg-gray-50 cursor-pointer"
                >
                  <p className="text-sm text-gray-600 truncate">{session.preview}</p>
                  <p className="text-xs text-gray-400 mt-1">
                    {new Date(session.modifiedAt).toLocaleString()}
                  </p>
                </div>
              ))}
            </div>
          </div>
        );
      })}
    </div>
  );
}
```

**Step 6: Test**

```bash
cd client && npm run dev
```
Expected: Click session → see rendered conversation with markdown

**Step 7: Commit**

```bash
git add .
git commit -m "feat: add conversation view with markdown rendering and tool badges"
```

---

## Task 6: HTML Export Feature

**Files:**
- Create: `client/src/utils/exportHtml.ts`
- Modify: `client/src/components/ConversationView.tsx`

**Step 1: Create export utility**

Create `client/src/utils/exportHtml.ts`:
```typescript
interface ToolCall {
  name: string;
  count: number;
}

interface MessageData {
  role: 'user' | 'assistant';
  content: string;
  toolCalls?: ToolCall[];
}

export function generateStandaloneHtml(messages: MessageData[]): string {
  const messagesHtml = messages.map(msg => {
    const isUser = msg.role === 'user';
    const avatar = isUser ? 'U' : 'C';
    const avatarBg = isUser ? '#3b82f6' : '#f97316';
    const bgColor = isUser ? '#ffffff' : '#f9fafb';
    const name = isUser ? 'You' : 'Claude';

    const contentHtml = isUser
      ? `<p style="white-space: pre-wrap; color: #1f2937;">${escapeHtml(msg.content)}</p>`
      : `<div class="markdown">${markdownToHtml(msg.content)}</div>`;

    const toolsHtml = msg.toolCalls?.length
      ? `<details style="margin-top: 8px;">
          <summary style="font-size: 12px; color: #6b7280; cursor: pointer;">
            🔧 ${msg.toolCalls.reduce((sum, t) => sum + t.count, 0)} tool calls
          </summary>
          <div style="padding-left: 16px; font-size: 12px; color: #6b7280;">
            ${msg.toolCalls.map(t => `<div>${t.name} × ${t.count}</div>`).join('')}
          </div>
        </details>`
      : '';

    return `
      <div style="padding: 24px 0; background: ${bgColor}; border-bottom: 1px solid #e5e7eb;">
        <div style="max-width: 768px; margin: 0 auto; padding: 0 16px;">
          <div style="display: flex; gap: 16px;">
            <div style="width: 32px; height: 32px; border-radius: 50%; background: ${avatarBg}; color: white; display: flex; align-items: center; justify-content: center; font-size: 14px; flex-shrink: 0;">
              ${avatar}
            </div>
            <div style="flex: 1; min-width: 0;">
              <p style="font-size: 14px; font-weight: 500; color: #374151; margin-bottom: 8px;">${name}</p>
              ${contentHtml}
              ${toolsHtml}
            </div>
          </div>
        </div>
      </div>
    `;
  }).join('');

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Claude Conversation</title>
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; }
    .markdown { color: #1f2937; line-height: 1.6; }
    .markdown p { margin-bottom: 16px; }
    .markdown pre { background: #f3f4f6; padding: 12px; border-radius: 8px; overflow-x: auto; margin: 12px 0; }
    .markdown code { font-family: 'SF Mono', Monaco, monospace; font-size: 14px; }
    .markdown :not(pre) > code { background: #f3f4f6; padding: 2px 6px; border-radius: 4px; }
    .markdown ul, .markdown ol { margin-left: 24px; margin-bottom: 16px; }
    .markdown h1, .markdown h2, .markdown h3 { font-weight: 600; margin: 16px 0 8px; }
    .markdown blockquote { border-left: 3px solid #d1d5db; padding-left: 12px; color: #6b7280; }
    @media print { body { font-size: 12px; } }
  </style>
</head>
<body>
  <header style="background: white; border-bottom: 1px solid #e5e7eb; padding: 16px;">
    <div style="max-width: 768px; margin: 0 auto;">
      <h1 style="font-size: 18px; font-weight: 600;">Claude Conversation</h1>
      <p style="font-size: 12px; color: #6b7280;">Exported ${new Date().toLocaleString()}</p>
    </div>
  </header>
  ${messagesHtml}
</body>
</html>`;
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function markdownToHtml(markdown: string): string {
  // Simple markdown conversion - handles common cases
  let html = escapeHtml(markdown);

  // Code blocks
  html = html.replace(/```(\w+)?\n([\s\S]*?)```/g, (_, lang, code) => {
    return `<pre><code class="language-${lang || ''}">${code.trim()}</code></pre>`;
  });

  // Inline code
  html = html.replace(/`([^`]+)`/g, '<code>$1</code>');

  // Bold
  html = html.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');

  // Italic
  html = html.replace(/\*([^*]+)\*/g, '<em>$1</em>');

  // Headers
  html = html.replace(/^### (.+)$/gm, '<h3>$1</h3>');
  html = html.replace(/^## (.+)$/gm, '<h2>$1</h2>');
  html = html.replace(/^# (.+)$/gm, '<h1>$1</h1>');

  // Lists
  html = html.replace(/^- (.+)$/gm, '<li>$1</li>');
  html = html.replace(/(<li>.*<\/li>\n?)+/g, '<ul>$&</ul>');

  // Paragraphs
  html = html.replace(/\n\n/g, '</p><p>');
  html = `<p>${html}</p>`;

  // Clean up empty paragraphs
  html = html.replace(/<p>\s*<\/p>/g, '');
  html = html.replace(/<p>(<h[123]>)/g, '$1');
  html = html.replace(/(<\/h[123]>)<\/p>/g, '$1');
  html = html.replace(/<p>(<pre>)/g, '$1');
  html = html.replace(/(<\/pre>)<\/p>/g, '$1');
  html = html.replace(/<p>(<ul>)/g, '$1');
  html = html.replace(/(<\/ul>)<\/p>/g, '$1');

  return html;
}

export function downloadHtml(html: string, filename: string): void {
  const blob = new Blob([html], { type: 'text/html' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}
```

**Step 2: Wire up export button**

Modify `client/src/components/ConversationView.tsx`, add import and update export button:

Add at top:
```tsx
import { generateStandaloneHtml, downloadHtml } from '../utils/exportHtml';
```

Update the export button onClick:
```tsx
<button
  onClick={() => {
    const html = generateStandaloneHtml(messages);
    downloadHtml(html, `claude-conversation-${sessionId}.html`);
  }}
  className="ml-auto px-3 py-1 bg-blue-500 text-white text-sm rounded hover:bg-blue-600"
>
  Export HTML
</button>
```

**Step 3: Test export**

```bash
cd client && npm run dev
```
1. Click a session
2. Click "Export HTML"
3. Open downloaded file in browser

Expected: Standalone HTML with Claude.ai-style formatting

**Step 4: Commit**

```bash
git add .
git commit -m "feat: add HTML export with embedded styles"
```

---

## Task 7: Production Build & npm Package

**Files:**
- Modify: `package.json` (root)
- Modify: `src/server.ts`

**Step 1: Update server to serve built frontend**

Modify `src/server.ts`:
```typescript
import express from 'express';
import getPort from 'get-port';
import open from 'open';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { getProjects } from './sessions.js';
import { parseSession } from './parser.js';
import { homedir } from 'os';

const __dirname = dirname(fileURLToPath(import.meta.url));

export async function startServer(): Promise<number> {
  const app = express();
  const port = await getPort({ port: [3000, 3001, 3002, 3003, 3004, 3005] });

  // API routes
  app.get('/api/projects', async (req, res) => {
    const projects = await getProjects();
    res.json(projects);
  });

  app.get('/api/session/:projectDir/:sessionId', async (req, res) => {
    const { projectDir, sessionId } = req.params;
    const filePath = join(homedir(), '.claude', 'projects', projectDir, `${sessionId}.jsonl`);

    try {
      const session = await parseSession(filePath);
      res.json(session);
    } catch (error) {
      res.status(404).json({ error: 'Session not found' });
    }
  });

  // Serve static frontend
  const clientPath = join(__dirname, 'client');
  app.use(express.static(clientPath));
  app.get('*', (req, res) => {
    res.sendFile(join(clientPath, 'index.html'));
  });

  return new Promise((resolve) => {
    app.listen(port, () => {
      const url = `http://localhost:${port}`;
      console.log(`✓ Claude Session Viewer running at ${url}`);
      open(url);
      resolve(port);
    });
  });
}
```

**Step 2: Update root package.json**

```json
{
  "name": "claude-view",
  "version": "1.0.0",
  "description": "Browse and export Claude Code sessions as beautiful HTML",
  "type": "module",
  "bin": {
    "claude-view": "./dist/cli.js"
  },
  "files": [
    "dist"
  ],
  "scripts": {
    "build": "npm run build:client && npm run build:server",
    "build:server": "tsc",
    "build:client": "cd client && npm run build",
    "start": "node dist/cli.js",
    "dev": "concurrently \"npm run dev:server\" \"npm run dev:client\"",
    "dev:server": "ts-node src/cli.ts",
    "dev:client": "cd client && npm run dev",
    "prepublishOnly": "npm run build"
  },
  "keywords": ["claude", "anthropic", "viewer", "export", "conversation"],
  "license": "MIT",
  "dependencies": {
    "express": "^4.18.2",
    "get-port": "^7.0.0",
    "open": "^10.0.0"
  },
  "devDependencies": {
    "@types/express": "^4.17.21",
    "@types/node": "^20.10.0",
    "concurrently": "^8.2.2",
    "ts-node": "^10.9.2",
    "typescript": "^5.3.2"
  }
}
```

**Step 3: Build everything**

```bash
npm install concurrently -D
cd client && npm install && npm run build
cd .. && npm run build
```

**Step 4: Test production build**

```bash
npm start
```
Expected: Browser opens with working app, no dev server needed

**Step 5: Test npx locally**

```bash
npm link
npx claude-view
```

**Step 6: Commit**

```bash
git add .
git commit -m "feat: production build with npm package setup"
```

---

## Task 8: Publish to npm

**Step 1: Login to npm**

```bash
npm login
```

**Step 2: Publish**

```bash
npm publish
```

**Step 3: Test installation**

```bash
npm unlink claude-view
npx claude-view
```

Expected: Downloads from npm, runs successfully

---

## Summary

After completing all tasks, users can run:
```bash
npx claude-view
```

And get:
- Auto-port selection
- Auto-browser open
- Session browser with previews
- Claude.ai-style conversation view
- Markdown rendering
- Collapsible tool badges
- Collapsible code blocks
- One-click HTML export
