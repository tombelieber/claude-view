---
status: pending
date: 2026-01-29
---

# UI Testing Strategy for 7-Type Conversation Redesign
## Solo Dev Testing Framework (Jest + RTL + CI/CD)

**Status:** Testing Specification (Pre-Implementation)
**Date:** 2026-01-29
**Context:** Starting from zero automation, solo developer, shadcn/ui + custom Tailwind mix
**Goal:** 80%+ coverage on 20+ new components with realistic effort

---

## 🎯 Testing Pyramid for This Project

```
                    E2E Tests (1-2 flows)          ← Skip for now
              Integration Tests (8-10 tests)        ← Critical path
         Unit Tests (60-80 tests)                   ← Main focus
    Type Safety + PropTypes (TypeScript)            ← Already covered
Manual Spot Checks (dark mode, mobile, browser)    ← Quick validation
```

**Why this pyramid:**
- Solo dev = automated tests must catch 90% of bugs
- E2E too slow/flaky for rapid iteration
- Unit + integration cover most scenarios
- Manual spot checks for edge cases typescript can't catch

---

## 📦 Phase 1: Test Infrastructure Setup

### 1.1 Install Dependencies

```bash
npm install --save-dev \
  @testing-library/react@14 \
  @testing-library/jest-dom@6 \
  @testing-library/user-event@14 \
  jest@29 \
  @babel/preset-react \
  @babel/preset-typescript \
  ts-jest@29 \
  identity-obj-proxy
```

### 1.2 Jest Configuration

**`jest.config.js`:**
```javascript
module.exports = {
  preset: 'ts-jest',
  testEnvironment: 'jsdom',
  roots: ['<rootDir>/src'],
  testMatch: ['**/__tests__/**/*.test.ts?(x)', '**/?(*.)+(spec|test).ts?(x)'],
  moduleNameMapper: {
    '^@/(.*)$': '<rootDir>/src/$1',
    '\\.(css|less|scss|sass)$': 'identity-obj-proxy',
  },
  collectCoverageFrom: [
    'src/components/**/*.{ts,tsx}',
    '!src/components/**/*.d.ts',
    '!src/**/*.stories.tsx',
  ],
  coverageThreshold: {
    global: {
      branches: 70,
      functions: 75,
      lines: 75,
      statements: 75,
    },
  },
  setupFilesAfterEnv: ['<rootDir>/src/setupTests.ts'],
};
```

### 1.3 Setup Files

**`src/setupTests.ts`:**
```typescript
import '@testing-library/jest-dom'

// Mock window.matchMedia for dark mode tests
Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: jest.fn().mockImplementation(query => ({
    matches: query.includes('dark') ? false : true,
    media: query,
    onchange: null,
    addListener: jest.fn(),
    removeListener: jest.fn(),
    addEventListener: jest.fn(),
    removeEventListener: jest.fn(),
    dispatchEvent: jest.fn(),
  })),
})

// Mock clipboard API
Object.assign(navigator, {
  clipboard: {
    writeText: jest.fn(() => Promise.resolve()),
  },
})
```

---

## 🏗️ Test Architecture

### Component Types & Test Patterns

```
20+ Components organized into 3 test categories:

1. XML Card Components (10 types)
   ├─ ToolCallCard (NEW)
   ├─ StructuredDataCard (NEW)
   └─ 8 existing cards (LocalCommand, TaskNotification, etc.)

2. System/Progress Event Cards (10 types)
   ├─ TurnDurationCard (NEW)
   ├─ ApiErrorCard (NEW)
   ├─ AgentProgressCard (NEW)
   └─ 7 more...

3. Message Container Components (2 types)
   ├─ MessageTyped (enhanced with threading)
   └─ ConversationThread (NEW - for hierarchy)
```

Each type gets:
- ✅ Unit tests (component renders, props)
- ✅ Integration tests (with parent MessageTyped)
- ⚠️ Manual spot checks (browser, mobile, dark mode)

---

## 🧪 Test Templates (Copy-Paste Ready)

### Template 1: Simple Card Component

```typescript
// src/components/__tests__/ToolCallCard.test.tsx
import { render, screen } from '@testing-library/react'
import { ToolCallCard } from '../ToolCallCard'

describe('ToolCallCard', () => {
  // ✅ HAPPY PATH: Normal rendering
  it('renders tool call with name and description', () => {
    render(
      <ToolCallCard
        name="Read"
        description="Reading file content"
        parameters={{ file_path: '/path/to/file.txt' }}
      />
    )

    expect(screen.getByText('Read')).toBeInTheDocument()
    expect(screen.getByText(/Reading file/)).toBeInTheDocument()
  })

  // ✅ COLLAPSED STATE: Should show summary, hide details
  it('starts collapsed and expands on click', () => {
    const { rerender } = render(
      <ToolCallCard
        name="Edit"
        description="File edit"
        parameters={{ file_path: '/path' }}
      />
    )

    // Details hidden initially
    expect(screen.queryByText(/old_string/)).not.toBeInTheDocument()

    // Click expand
    const expandBtn = screen.getByRole('button', { name: /Edit/ })
    fireEvent.click(expandBtn)

    // Rerender with expanded state
    rerender(
      <ToolCallCard
        name="Edit"
        description="File edit"
        parameters={{ file_path: '/path' }}
        expanded={true}
      />
    )

    // Details now visible
    expect(screen.getByText(/old_string/)).toBeInTheDocument()
  })

  // ✅ ERROR CASE: Missing parameters
  it('renders gracefully with missing parameters', () => {
    render(
      <ToolCallCard
        name="Bash"
        description="Execute command"
        parameters={undefined}
      />
    )

    expect(screen.getByText('Bash')).toBeInTheDocument()
    // Should not crash, no orphaned refs
  })

  // ✅ ACCESSIBILITY: Icon button has aria-label
  it('has accessible aria labels', () => {
    render(
      <ToolCallCard
        name="Write"
        description="Create file"
        parameters={{ file_path: '/new/file.txt' }}
      />
    )

    const expandBtn = screen.getByRole('button', { name: /Expand/ })
    expect(expandBtn).toHaveAttribute('aria-expanded')
  })

  // ✅ LONG TEXT: Should truncate or wrap properly
  it('handles very long file paths', () => {
    const longPath = '/very/long/path'.repeat(10)
    const { container } = render(
      <ToolCallCard
        name="Read"
        description="Long path test"
        parameters={{ file_path: longPath }}
      />
    )

    // Check no horizontal scroll
    const card = container.querySelector('[class*="card"]')
    expect(card).toHaveStyle({ overflow: 'hidden' })
  })
})
```

### Template 2: Event Card with Metadata

```typescript
// src/components/__tests__/AgentProgressCard.test.tsx
import { render, screen } from '@testing-library/react'
import { AgentProgressCard } from '../AgentProgressCard'

describe('AgentProgressCard', () => {
  const defaultProps = {
    agentId: 'agent-123',
    prompt: 'Implement feature X',
    model: 'claude-opus',
    tokens: { input: 500, output: 1200 },
    normalizedMessages: 15,
  }

  // ✅ HAPPY PATH: Render with all fields
  it('displays agent task with full metadata', () => {
    render(<AgentProgressCard {...defaultProps} />)

    expect(screen.getByText(/agent-123/i)).toBeInTheDocument()
    expect(screen.getByText(/Implement feature X/)).toBeInTheDocument()
    expect(screen.getByText(/1700 tokens/)).toBeInTheDocument()  // 500+1200
  })

  // ✅ EDGE CASE: Partial metadata
  it('renders with missing optional fields', () => {
    render(
      <AgentProgressCard
        agentId="agent-456"
        prompt="Simple task"
        model="claude-haiku"
        // tokens omitted
      />
    )

    expect(screen.getByText(/agent-456/)).toBeInTheDocument()
    expect(screen.queryByText(/tokens/)).not.toBeInTheDocument()
  })

  // ✅ INDENTATION: Should apply correct level
  it('applies correct indentation for nesting', () => {
    const { container } = render(
      <AgentProgressCard {...defaultProps} indent={24} />
    )

    const card = container.firstChild
    expect(card).toHaveStyle({ paddingLeft: '24px' })
  })

  // ✅ ICON: Should have robot icon with aria-hidden
  it('has decorative robot icon hidden from screen readers', () => {
    const { container } = render(<AgentProgressCard {...defaultProps} />)

    const icon = container.querySelector('svg')
    expect(icon).toHaveAttribute('aria-hidden', 'true')
  })
})
```

### Template 3: Container with Threading

```typescript
// src/components/__tests__/MessageTyped.test.tsx
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MessageTyped } from '../MessageTyped'

describe('MessageTyped with Threading', () => {
  const mockMessage = {
    role: 'user',
    content: 'Hello, Claude',
    timestamp: '2026-01-29T10:00:00Z',
  }

  // ✅ BASIC RENDER: Message type badge
  it('renders correct type badge for message role', () => {
    render(
      <MessageTyped
        message={mockMessage}
        messageType="user"
      />
    )

    expect(screen.getByText('You')).toBeInTheDocument()
    expect(screen.getByText('Hello, Claude')).toBeInTheDocument()
  })

  // ✅ THREADING: Shows parent relationship
  it('displays thread indent for child messages', () => {
    const { container } = render(
      <MessageTyped
        message={mockMessage}
        messageType="assistant"
        parentUuid="user-msg-1"
        indent={12}
      />
    )

    const card = container.firstChild
    expect(card).toHaveClass('border-l-4')  // Left border accent
    expect(card).toHaveStyle({ marginLeft: '12px' })
  })

  // ✅ HOVER: Highlight thread chain
  it('highlights message on hover', async () => {
    const user = userEvent.setup()
    const { container } = render(
      <MessageTyped message={mockMessage} messageType="user" />
    )

    const card = container.firstChild as HTMLElement
    await user.hover(card)

    expect(card).toHaveClass('hover:bg-gray-50')
  })

  // ✅ COPY BUTTON: Clipboard functionality
  it('copies message to clipboard on button click', async () => {
    const user = userEvent.setup()
    render(
      <MessageTyped message={mockMessage} messageType="user" />
    )

    const copyBtn = screen.getByRole('button', { name: /copy/i })
    await user.click(copyBtn)

    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
      'Hello, Claude'
    )
  })

  // ✅ KEYBOARD NAV: Tab order respects tree
  it('supports keyboard navigation through messages', async () => {
    const user = userEvent.setup()
    const { container } = render(
      <MessageTyped message={mockMessage} messageType="user" />
    )

    const copyBtn = screen.getByRole('button', { name: /copy/i })

    await user.tab()
    expect(copyBtn).toHaveFocus()
  })

  // ✅ ERROR STATE: Graceful handling of bad data
  it('handles undefined content gracefully', () => {
    render(
      <MessageTyped
        message={{ ...mockMessage, content: undefined }}
        messageType="system"
        metadata={{ durationMs: 100 }}
      />
    )

    // Should show metadata instead
    expect(screen.getByText(/durationMs/)).toBeInTheDocument()
  })
})
```

### Template 4: Integration Test (Message → XML Card)

```typescript
// src/components/__tests__/MessageWithXmlIntegration.test.tsx
import { render, screen } from '@testing-library/react'
import { MessageTyped } from '../MessageTyped'

describe('MessageTyped + XML Card Integration', () => {
  // ✅ INTEGRATION: Message renders correct XML card
  it('renders observation XML as ObservationCard (not codeblock)', () => {
    const messageWithXml = {
      role: 'assistant',
      content: `Here's my observation:

<observation>
  <type>Code Analysis</type>
  <title>Found bug in loop</title>
  <facts>
    <fact>Array bounds check missing</fact>
    <fact>Off-by-one error on line 42</fact>
  </facts>
</observation>`,
      timestamp: '2026-01-29T10:05:00Z',
    }

    render(
      <MessageTyped
        message={messageWithXml}
        messageType="assistant"
      />
    )

    // Should render as semantic card, NOT codeblock
    expect(screen.getByText('Code Analysis')).toBeInTheDocument()
    expect(screen.getByText(/Found bug/)).toBeInTheDocument()
    expect(screen.getByText(/Array bounds/)).toBeInTheDocument()

    // Verify no <pre> tag (which would indicate codeblock)
    expect(screen.queryByRole('region', { name: /codeblock/i })).not.toBeInTheDocument()
  })

  // ✅ INTEGRATION: tool_call renders ToolCallCard
  it('renders tool_call XML as ToolCallCard (NEW)', () => {
    const messageWithToolCall = {
      role: 'assistant',
      content: `<tool_call>
        <what_happened>Read file</what_happened>
        <parameters>{"file_path": "/src/main.ts"}</parameters>
      </tool_call>`,
      timestamp: '2026-01-29T10:10:00Z',
    }

    render(
      <MessageTyped
        message={messageWithToolCall}
        messageType="assistant"
      />
    )

    expect(screen.getByText(/Read file/)).toBeInTheDocument()
    expect(screen.getByText(/main.ts/)).toBeInTheDocument()
  })

  // ✅ INTEGRATION: Multiple XML cards in one message
  it('renders multiple XML cards inline', () => {
    const message = {
      role: 'assistant',
      content: `First I'll analyze:

<observation>
  <type>Discovery</type>
  <title>Found pattern</title>
</observation>

Then I'll act:

<tool_call>
  <what_happened>Read file</what_happened>
  <parameters>{"file_path": "/path"}</parameters>
</tool_call>`,
      timestamp: '2026-01-29T10:15:00Z',
    }

    render(
      <MessageTyped message={message} messageType="assistant" />
    )

    // Both cards should render
    expect(screen.getByText(/Found pattern/)).toBeInTheDocument()
    expect(screen.getByText(/Read file/)).toBeInTheDocument()
  })
})
```

---

## 📋 Test Coverage Map

### XML Card Components (10 types)

| Component | Unit Tests | Integration | Notes |
|-----------|-----------|-------------|-------|
| ToolCallCard | 5 | 2 | NEW - critical |
| StructuredDataCard | 4 | 1 | NEW - fallback |
| ObservedFromPrimarySession | 4 | 1 | Existing, enhance |
| Observation | 4 | 1 | Existing, enhance |
| LocalCommand | 3 | 1 | Existing, verify |
| TaskNotification | 4 | 1 | Existing, enhance |
| Command | 4 | 1 | Existing, verify |
| ToolError | 3 | 1 | Existing, verify |
| UntrustedData | 3 | 1 | Existing, verify |
| **TOTAL** | **38** | **10** | **48 tests** |

### Event Card Components (10 types)

| Component | Unit Tests | Integration | Notes |
|-----------|-----------|-------------|-------|
| TurnDurationCard | 3 | 1 | NEW |
| ApiErrorCard | 4 | 1 | NEW |
| CompactBoundaryCard | 3 | 1 | NEW |
| HookSummaryCard | 4 | 1 | NEW |
| LocalCommandEventCard | 2 | 1 | NEW |
| AgentProgressCard | 5 | 2 | NEW - complex |
| BashProgressCard | 4 | 1 | NEW |
| HookProgressCard | 3 | 1 | NEW |
| McpProgressCard | 3 | 1 | NEW |
| TaskQueueCard | 3 | 1 | NEW |
| **TOTAL** | **34** | **11** | **45 tests** |

### Message Container Components

| Component | Unit Tests | Integration | Notes |
|-----------|-----------|-------------|-------|
| MessageTyped (enhanced) | 8 | 4 | Enhanced with threading |
| MessageQueueEventCard | 3 | 1 | NEW - queue ops |
| FileSnapshotCard | 3 | 1 | NEW - snapshots |
| SessionSummaryCard | 3 | 1 | NEW - summaries |
| **TOTAL** | **17** | **7** | **24 tests** |

### Grand Total

```
Unit Tests:        89 tests (~4-5 hours writing)
Integration Tests: 28 tests (~2-3 hours writing)
Edge Cases:        15 tests (~1 hour writing)
─────────────────────────────
TOTAL:            132 tests (~8 hours writing)
Coverage Target:   80%+
Run Time:          ~30 seconds (full suite)
```

---

## 🔄 CI/CD Pipeline (GitHub Actions)

**`.github/workflows/test.yml`:**
```yaml
name: Test UI Components

on:
  push:
    branches: [main, develop]
    paths:
      - 'src/components/**'
      - 'src/**/__tests__/**'
  pull_request:
    branches: [main]
    paths:
      - 'src/components/**'
      - 'src/**/__tests__/**'

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Setup Node.js
        uses: actions/setup-node@v3
        with:
          node-version: '18'
          cache: 'npm'

      - name: Install dependencies
        run: npm ci

      - name: Run tests
        run: npm test -- --coverage --watchAll=false

      - name: Check coverage
        run: |
          COVERAGE=$(npm test -- --coverage --watchAll=false 2>&1 | grep "Statements" | awk '{print $3}' | cut -d'%' -f1)
          if (( $(echo "$COVERAGE < 80" | bc -l) )); then
            echo "Coverage ${COVERAGE}% is below 80% threshold"
            exit 1
          fi

      - name: Upload coverage
        uses: codecov/codecov-action@v3
        with:
          files: ./coverage/lcov.info
          fail_ci_if_error: false

  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Setup Node.js
        uses: actions/setup-node@v3
        with:
          node-version: '18'
          cache: 'npm'

      - name: Install dependencies
        run: npm ci

      - name: Run ESLint
        run: npm run lint -- --fix-dry-run src/components/**/*.tsx
```

**`package.json` scripts:**
```json
{
  "scripts": {
    "test": "jest",
    "test:watch": "jest --watch",
    "test:coverage": "jest --coverage",
    "test:ui": "jest src/components --watch",
    "lint": "eslint --ext .ts,.tsx src"
  }
}
```

---

## ✅ Manual Testing Checklist

**For each component, test manually in browser (30 min per day):**

### Dark Mode Testing
```bash
# In DevTools console:
document.documentElement.setAttribute('data-theme', 'dark')
```
- [ ] Text contrast sufficient (not washed out)
- [ ] Icons visible (not blending with background)
- [ ] Borders visible (not too subtle)
- [ ] No harsh white glows

### Mobile Testing (375px viewport)
- [ ] No horizontal scroll
- [ ] Touch targets 44x44px minimum
- [ ] Text readable (not too small)
- [ ] Indentation preserved (not crushed)

### Browser Testing
- [ ] Chrome/Chromium latest
- [ ] Firefox latest
- [ ] Safari latest (macOS)
- [ ] Mobile Safari (iOS)

### Keyboard Navigation
```
- [ ] Tab moves through interactive elements
- [ ] Shift+Tab goes backward
- [ ] Enter activates buttons
- [ ] Arrow keys navigate lists/trees (if implemented)
- [ ] Escape closes collapsibles
```

### Content Edge Cases
- [ ] Very long text (>500 chars) doesn't break layout
- [ ] Empty content shows appropriate placeholder
- [ ] Special characters (emoji, RTL) render correctly
- [ ] URLs in text are clickable
- [ ] Code blocks with syntax highlighting work

---

## 🚀 Testing Timeline (Solo Dev)

```
Week 1:
  Mon-Tue   → Jest setup + test infrastructure (2h)
  Wed-Thu   → Write 40 unit tests (XML cards) (4h)
  Fri       → Write 10 integration tests + CI/CD setup (2h)

Week 2:
  Mon-Tue   → Write 34 unit tests (event cards) (4h)
  Wed-Thu   → Write 11 integration tests (2h)
  Fri       → Manual spot checks + coverage review (2h)

Week 3:
  Mon-Wed   → Fix coverage gaps + edge cases (4h)
  Thu       → Final manual testing (mobile, dark, keyboard) (3h)
  Fri       → Ready for Phase 1 implementation
```

**Total: ~24 hours testing setup** (before coding)

---

## 📊 Success Criteria

### Automated Tests
- ✅ 80%+ line coverage on all new components
- ✅ All 132 tests passing
- ✅ Zero console errors/warnings
- ✅ CI/CD pipeline passes on all PRs

### Manual Testing
- ✅ All components tested in 3 browsers
- ✅ Dark mode verified (text, icons, borders)
- ✅ Mobile 375px verified
- ✅ Keyboard navigation works (Tab, Arrow, Enter, Escape)

### Quality Gates
- ✅ No flaky tests (>99% pass rate)
- ✅ Tests run in <30 seconds
- ✅ TypeScript strict mode passes
- ✅ No accessibility violations (manual spot check)

---

## 🛠️ Testing Tools Reference

| Tool | Purpose | Cost |
|------|---------|------|
| Jest | Unit testing framework | Free |
| React Testing Library | Component testing | Free |
| @testing-library/user-event | Realistic user interactions | Free |
| GitHub Actions | CI/CD pipeline | Free (public repos) |
| Codecov | Coverage tracking | Free tier |

**Total cost: $0** (all open source)

---

## 📚 Reference Docs

- `docs/2026-01-29-CONVERSATION-UI-COMPREHENSIVE-REDESIGN.md` — Component specs
- `src/components/__tests__/` — Test file location
- `.github/workflows/test.yml` — CI/CD pipeline
- `jest.config.js` — Test configuration

---

**Status:** Ready for approval
**Next Step:** Approve testing plan → Setup Jest → Write tests → Implement components
