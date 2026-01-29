# Quick Start: 7-Type Message UI Implementation

**For developers integrating the new message card system. 5-minute setup.**

---

## TL;DR

Replace `Message` with `MessageTyped` in SessionView.tsx. Pass `messageType` for system/progress events. Done.

---

## Step 1: Import Component

```tsx
import { MessageTyped } from '@/components/MessageTyped'
```

## Step 2: Replace in SessionView.tsx

**Before:**
```tsx
{session.messages.map((msg, idx) => (
  <Message key={idx} message={msg} messageIndex={idx} />
))}
```

**After:**
```tsx
{session.messages.map((msg, idx) => (
  <MessageTyped key={idx} message={msg} messageIndex={idx} />
))}
```

## Step 3: Add Type Support (When Backend Ready)

```tsx
// When API returns message_type and metadata
{session.messages.map((msg, idx) => (
  <MessageTyped
    key={idx}
    message={msg}
    messageIndex={idx}
    messageType={msg.type} // 'user' | 'assistant' | 'system' | etc.
    metadata={msg.metadata} // { duration_ms: 245, ... }
  />
))}
```

---

## Component Props

```typescript
interface MessageTypedProps {
  message: MessageType                    // Required
  messageIndex?: number                   // Optional (for code block IDs)
  messageType?: 'user' | 'assistant' | 'tool_use' | 'tool_result' | 'system' | 'progress' | 'summary'
  metadata?: Record<string, any>          // Optional (for system/progress events)
}
```

---

## Message Type Legend

| Type | Icon | Use Case | Example |
|------|------|----------|---------|
| **user** | User | User prompts, tool results | "Tell me about..." |
| **assistant** | Message | Model responses | "I recommend..." |
| **tool_use** | Wrench | Tool invocation | Read file at `/path/to/file` |
| **tool_result** | CheckCircle | Tool output | File contents returned |
| **system** | AlertCircle | System events | Turn duration, errors |
| **progress** | Zap | Agent activity | Agent spawned, bash running |
| **summary** | BookOpen | Session summary | Auto-generated recap |

---

## System Event Metadata Example

```tsx
// When rendering a system event
<MessageTyped
  message={{
    role: 'assistant',
    content: '', // No text content
    timestamp: '2026-01-29T14:35:22Z'
  }}
  messageType="system"
  metadata={{
    duration_ms: 1245,
    api_error_count: 1,
    retry_count: 0,
    hook_blocked_count: 2
  }}
/>
```

**Renders:**
```
┌─ Amber-300 border ─────────────────────┐
│ ⚠  System                  [📋] [2:35] │
│ duration_ms: 1245                     │
│ api_error_count: 1                    │
│ retry_count: 0                        │
│ hook_blocked_count: 2                 │
└───────────────────────────────────────┘
```

---

## Progress Event Metadata Example

```tsx
<MessageTyped
  message={{
    role: 'assistant',
    content: 'Starting parallel indexing...',
    timestamp: '2026-01-29T14:36:00Z'
  }}
  messageType="progress"
  metadata={{
    agent_spawn_count: 3,
    bash_progress_count: 5,
    mcp_progress_count: 0,
    hook_progress_count: 1
  }}
/>
```

---

## Color Guide (Tailwind Classes)

MessageTyped handles all styling internally. Reference for custom work:

```
user:       border-blue-300,      bg-blue-100
assistant:  border-orange-300,    bg-orange-100
tool_use:   border-purple-300,    bg-purple-100
tool_result:border-green-300,     bg-green-100
system:     border-amber-300,     bg-amber-100
progress:   border-indigo-300,    bg-indigo-100
summary:    border-rose-300,      bg-rose-100
```

---

## Style Guide Reference

For detailed specs, see:
- **Full Style Guide:** `docs/STYLE_GUIDE_7TYPE_UI.md`
- **Design Philosophy:** `docs/DESIGN_PHILOSOPHY_7TYPE_UI.md`
- **Full Audit:** `docs/AUDIT_REDESIGN_SUMMARY.md`

---

## Backward Compatibility

MessageTyped automatically falls back to `message.role` if `messageType` is not provided:

```tsx
// Works without messageType (uses message.role)
<MessageTyped message={msg} />

// Explicit type override
<MessageTyped message={msg} messageType="system" />
```

---

## Performance Tips

For conversations with 500+ messages, add virtual scrolling:

```tsx
import { useVirtualizer } from '@tanstack/react-virtual'

const virtualizer = useVirtualizer({
  count: session.messages.length,
  getScrollElement: () => scrollRef.current,
  estimateSize: () => 100, // avg message height
})
```

---

## Testing Checklist

- [ ] MessageTyped renders without errors
- [ ] All 7 types display correct colors
- [ ] Icons match TYPE_CONFIG
- [ ] Metadata cards show for system/progress
- [ ] XML cards still extract correctly
- [ ] Thinking blocks render
- [ ] Tool calls badges display
- [ ] Hover states work (background change)
- [ ] Copy button appears on hover
- [ ] Links are blue, clickable
- [ ] Timestamps format correctly

---

## Common Issues

### Issue: Type colors not showing
**Check:** Ensure Tailwind colors are in `tailwind.config.js`
```js
extend: {
  colors: {
    'blue-300': '#93c5fd',
    'orange-300': '#fdba74',
    // ... all 7 colors
  }
}
```

### Issue: Icons not appearing
**Check:** Ensure lucide-react is installed
```bash
npm install lucide-react
```

### Issue: Metadata card not showing
**Check:** Pass `metadata` prop and ensure `messageType` is set to 'system' or 'progress'

### Issue: Old Message component still showing
**Check:** All imports are updated to `MessageTyped`
```tsx
// Old (remove)
import { Message } from '@/components/Message'

// New (add)
import { MessageTyped } from '@/components/MessageTyped'
```

---

## What MessageTyped Does

1. ✅ Renders 7 semantic message types with distinct colors
2. ✅ Shows icon badges (32×32px, type-specific)
3. ✅ Displays left border accent (4px, type-specific color)
4. ✅ Extracts and renders XML cards (unchanged from Message)
5. ✅ Renders thinking blocks
6. ✅ Shows tool calls summary
7. ✅ Displays metadata cards (for system/progress)
8. ✅ Handles timestamps and copy buttons
9. ✅ Supports dense layout (500+ messages)
10. ✅ Maintains backward compatibility with Message

---

## What MessageTyped Doesn't Do

- ❌ Change the parser (use Full 7-Type Parser separately)
- ❌ Handle virtual scrolling (use TanStack Virtual)
- ❌ Search highlighting (implement separately)
- ❌ Message selection (implement separately)

---

## Support

For questions about the design or implementation:
- See `docs/STYLE_GUIDE_7TYPE_UI.md` for detailed specs
- See `docs/AUDIT_REDESIGN_SUMMARY.md` for design rationale
- See `src/components/MessageTyped.tsx` for implementation details

---

**Ready to integrate!** 🚀
