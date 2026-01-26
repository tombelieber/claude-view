# PDF Export Feature Design

**Date:** 2026-01-27
**Status:** Ready for implementation

## Problem

Users want to share Claude conversations with non-tech people (via WhatsApp, for teaching, etc.). HTML files look suspicious ("virus") to recipients. PDF is universally trusted.

## Solution

Add PDF export alongside existing HTML export with two visible buttons.

## UI Design

```
┌──────────────────────────────────────────────────────┐
│  ← Back    Session Title              [HTML] [PDF]   │
└──────────────────────────────────────────────────────┘
```

- Two small outline buttons, side by side
- Both always visible - no dropdown, no hidden menus
- One click each, lowest friction

## Implementation

### Task 1: Add PDF Export Function

**File:** `src/lib/export-html.ts` (rename to `export.ts` or keep as-is)

```typescript
export function exportToPdf(messages: Message[]): void {
  const html = generateStandaloneHtml(messages)
  const printWindow = window.open('', '_blank')
  if (printWindow) {
    printWindow.document.write(html)
    printWindow.document.close()
    printWindow.print()
  }
}
```

**Why new window:**
- Doesn't disrupt current view
- Print dialog is isolated
- User can close without losing their place

### Task 2: Update ConversationView Header

**File:** `src/components/ConversationView.tsx`

Add two buttons to the header:

```tsx
<Button variant="outline" size="sm" onClick={handleExportHtml}>
  HTML
</Button>
<Button variant="outline" size="sm" onClick={handleExportPdf}>
  PDF
</Button>
```

Handler functions:
```tsx
const handleExportHtml = () => {
  const html = generateStandaloneHtml(messages)
  downloadHtml(html, `conversation-${sessionId}.html`)
}

const handleExportPdf = () => {
  exportToPdf(messages)
}
```

### Task 3: Add Keyboard Shortcuts

**File:** `src/components/ConversationView.tsx` or global shortcut handler

- `Cmd+Shift+E` → Export HTML
- `Cmd+Shift+P` → Export PDF

Use existing keyboard handling pattern in the codebase.

## Technical Notes

- **Zero new dependencies** - uses browser's native print-to-PDF
- **Existing print styles** - `export-html.ts` already has `@media print` CSS
- **~30 lines of new code** total

## Not In Scope

- Library-based PDF generation (jspdf, puppeteer)
- Server-side PDF rendering
- Custom PDF styling beyond existing print CSS
- Batch export multiple conversations

## Success Criteria

1. User can click "PDF" button in conversation view
2. Browser print dialog opens with conversation content
3. User can save as PDF from print dialog
4. PDF looks clean and professional (existing print styles)
