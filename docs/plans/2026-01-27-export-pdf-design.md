---
status: pending
date: 2026-01-27
---

# PDF Export Feature Design

**Date:** 2026-01-27
**Status:** Pending — browser print-to-PDF, zero new dependencies

## Problem

Users want to share Claude conversations with non-tech people (via WhatsApp, for teaching, etc.). HTML files look suspicious ("virus") to recipients. PDF is universally trusted.

## Solution

Add PDF export alongside existing markdown export with two visible buttons.

## UI Design

```
┌──────────────────────────────────────────────────────┐
│  ← Back    Session Title              [MD] [PDF]     │
└──────────────────────────────────────────────────────┘
```

- Two small outline buttons, side by side
- Both always visible - no dropdown, no hidden menus
- One click each, lowest friction

## Implementation

### Task 1: Add PDF Export Function

Open a new window with the conversation HTML and trigger the browser's native print dialog. Uses DOM manipulation (createElement/appendChild) to build the document safely — no innerHTML or document.write.

**Why new window:**
- Doesn't disrupt current view
- Print dialog is isolated
- User can close without losing their place

### Task 2: Update ConversationView Header

Add PDF button alongside existing markdown export.

### Task 3: Add Keyboard Shortcut

- `Cmd+Shift+P` → Export PDF

Use existing keyboard handling pattern in the codebase.

## Technical Notes

- **Zero new dependencies** - uses browser's native print-to-PDF
- **~30 lines of new code** total
- **Safe DOM construction** - use createElement/appendChild, not innerHTML

## Not In Scope

- Library-based PDF generation (jspdf, puppeteer)
- Server-side PDF rendering
- Custom PDF styling beyond existing print CSS
- Batch export multiple conversations

## Success Criteria

1. User can click "PDF" button in conversation view
2. Browser print dialog opens with conversation content
3. User can save as PDF from print dialog
4. PDF looks clean and professional
