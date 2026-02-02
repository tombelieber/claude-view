# Bulletproof Hardening Implementation Plan v2.0
## 7 Security + Robustness Fixes (TDD-First, Production-Grade)

**Status:** DONE — All 7 fixes implemented and tested (see `2026-02-02-hardening-final.md` for consolidation)
**Duration:** 9-10 hours over 3 days (realistic with debugging buffer)  
**Total Fixes:** 7 (Security, Error Handling, Robustness)  
**Tests:** 25+ with 80%+ coverage  
**Exit Criteria:** 100% production-ready

---

## 📋 Pre-Flight Checklist (30 minutes - Do THIS FIRST)

Before any code work, complete these 10 checks:

```bash
# 1. Node version check
node --version  # Must be v14.0.0+

# 2. Clean npm install
rm -rf node_modules package-lock.json
npm ci

# 3. Install DOMPurify
npm install dompurify
npm list dompurify

# 4. Test DOMPurify import
node -e "const DOMPurify = require('dompurify'); console.log('✓ Works')"

# 5. Verify TypeScript
npx tsc --noEmit

# 6. Baseline tests
npm test -- --passWithNoTests

# 7. Create feature branch
git checkout -b hardening/security-robustness

# 8. Verify pre-commit hooks
git commit --dry-run -m "test"

# 9. Document hook list
# Note any strict linting or typing rules

# 10. Ready to start
echo "✅ Pre-flight checks complete"
```

---

## 📅 DAY 1: Security Hardening (3.5 hours)

### Fix #1: DOMPurify + StructuredDataCard (1.5h)

**1.1 Create Component Stub (10 min)**

```typescript
// src/components/StructuredDataCard.tsx
export function StructuredDataCard({ xml, type = 'unknown' }) {
  return null  // Empty stub - tests will import this
}
```

**1.2 Write Tests (30 min)**

5 tests covering:
- Script tag sanitization ✅
- Event handler removal ✅
- Safe content preservation ✅
- Empty content handling ✅
- Large XML performance ✅

**1.3 Implement with DOMPurify (40 min)**

```typescript
import DOMPurify from 'dompurify'

const sanitizedXml = DOMPurify.sanitize(xml, {
  ALLOWED_TAGS: ['div', 'span', 'p', 'br', 'ul', 'ol', 'li', 'pre', 'code'],
  ALLOWED_ATTR: [],  // No attributes = no onclick/onerror
  KEEP_CONTENT: true,
})
// Render in <pre> as plaintext
```

**1.4 Verify Tests Pass**
```bash
npm test StructuredDataCard
# Expected: 5/5 tests PASS ✅
```

### Fix #2: UntrustedData XSS (1h)

**2.1 Write Tests (15 min)**
- Plaintext rendering only ✅
- No script execution ✅
- Formatting preserved ✅

**2.2 Update XmlCard.tsx (40 min)**
- Replace HTML rendering with plaintext <pre>
- Apply DOMPurify with ALLOWED_TAGS: []

**2.3 Manual Browser Test (5 min)**

### Fix #3: AgentProgressCard XSS (30 min)

**3.1 Write Test (10 min)**
- Verify React auto-escaping works

**3.2 Add JSDoc Comment (15 min)**
- Document that React escapes text by default

### Day 1 End Checklist

```bash
npm test -- --coverage
# Expected: 12 tests, 80%+ coverage

git commit -m "fix(security): XSS sanitization with DOMPurify"
```

---

## 📅 DAY 2: Error Handling + Nesting (3.5 hours)

### Start of Day: Regression Check (10 min)

```bash
npm test -- --testPathPattern="StructuredDataCard|XmlCard"
# Expected: Day 1 tests STILL pass ✅
```

### Fix #4: ErrorBoundary (1.5h)

**4.1 Create ErrorBoundary Component**

Catches component errors and shows fallback UI:
```typescript
static getDerivedStateFromError(error) {
  return { hasError: true, error }
}

componentDidCatch(error, errorInfo) {
  console.error('Error:', error)
}

render() {
  if (this.state.hasError) {
    return <div>Failed to render message</div>
  }
  return this.props.children
}
```

**4.2 Write 4 Tests**
- Catches errors ✅
- Uses custom fallback ✅
- Passes through when OK ✅
- Calls onError callback ✅

**4.3 Integrate with MessageTyped**

Wrap each message:
```typescript
<ErrorBoundary key={msg.uuid}>
  <MessageTyped message={msg} />
</ErrorBoundary>
```

### Fix #5: Max Nesting Depth (2h)

**5.1 Write Tests**
- Caps indent at 96px ✅
- Real nested tree (10+ levels) ✅
- Logs warning ✅

**5.2 Implement in MessageTyped**

```typescript
const MAX_INDENT_LEVEL = 8
const calculatedIndent = Math.min(indent, MAX_INDENT_LEVEL * 12)
if (indent > MAX_INDENT_LEVEL * 12) {
  console.warn('Max nesting exceeded')
}
```

### Day 2 End Checklist

```bash
npm test -- --coverage
# Expected: 20 tests, 80%+ coverage

git commit -m "fix(robustness): error boundary + max nesting"
```

---

## 📅 DAY 3: Robustness + Cleanup (2.5 hours)

### Start of Day: Regression Check (10 min)

```bash
npm test  # All 20 Day 1-2 tests still pass ✅
```

### Fix #6: Null/Undefined Handling (1.5h)

**Template for all 10 components:**

```typescript
function ComponentName({ requiredField = null, optional }) {
  // Validate
  if (!requiredField) {
    return <div>Data unavailable</div>
  }
  // Render safely
  return <div>{optional ? optional : 'No data'}</div>
}
```

Apply to:
1. TurnDurationCard
2. ApiErrorCard
3. CompactBoundaryCard
4. HookSummaryCard
5. LocalCommandEventCard
6. AgentProgressCard
7. BashProgressCard
8. HookProgressCard
9. McpProgressCard
10. TaskQueueCard

### Fix #7: useEffect Cleanup (30 min)

**Pattern:**

```typescript
useEffect(() => {
  const handler = () => {}
  window.addEventListener('scroll', handler)
  
  // Cleanup removes listener
  return () => {
    window.removeEventListener('scroll', handler)
  }
}, [])
```

Search and fix all components with listeners.

### Day 3 Final Checklist

```bash
# All tests pass with coverage
npm test -- --coverage
# Expected: 25+ tests, 80%+ lines, 75%+ branches

# No errors
npm run lint && npx tsc --noEmit

# Manual browser test
npm run dev
# Test error boundary triggers, nesting renders

# Final commit
git commit -m "fix(production): null safety + cleanup patterns"

# Log commits
git log --oneline | head -3
# Should show 3 feature commits
```

---

## ✅ Completion Checklist

```
Security (3 fixes)
☑ DOMPurify sanitization working
☑ UntrustedData plaintext only
☑ React auto-escape verified

Error Handling (2 fixes)
☑ ErrorBoundary catches crashes
☑ Max nesting enforced (8 levels)

Robustness (2 fixes)
☑ All 10 components handle null/undefined
☑ useEffect cleanup on unmount

Testing
☑ 25+ tests total
☑ 80%+ code coverage
☑ All tests passing
☑ <30 second runtime

Quality
☑ ESLint: 0 violations
☑ TypeScript: 0 errors
☑ Console: 0 errors
☑ Manual testing: Complete

Process
☑ Feature branch used
☑ Daily regression checks passed
☑ 3 feature commits
☑ Pre-commit hooks passed
```

---

## 🎯 After Hardening

**Status:** 100% Production-Ready

```
Day 4 (Thursday):
  ✓ Final review
  ✓ Merge branch: git merge --squash hardening/security-robustness
  ✓ Update PROGRESS.md

Day 5+ (Friday):
  ✓ Begin Phase 1 implementation
  ✓ Use test templates from testing strategy
  ✓ Build with confidence
```

---

**Ready to Start Monday? ✅**

