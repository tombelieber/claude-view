#!/bin/bash
# verify-test-env.sh
# Verify test environment is ready for E2E testing

set -e

echo "🔍 Session Discovery E2E Test Environment Verification"
echo "======================================================"
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

PASS="${GREEN}✅ PASS${NC}"
FAIL="${RED}❌ FAIL${NC}"
WARN="${YELLOW}⚠️  WARN${NC}"

# Track overall status
ALL_PASS=true

# Check 1: Backend server
echo -n "1. Backend server (http://localhost:47892)... "
if curl -s http://localhost:47892/api/health > /dev/null 2>&1; then
    echo -e "$PASS"
else
    echo -e "$FAIL"
    echo "   → Start backend: cargo run -p server"
    ALL_PASS=false
fi

# Check 2: Frontend dev server
echo -n "2. Frontend dev server (http://localhost:5173)... "
if curl -s http://localhost:5173 > /dev/null 2>&1; then
    echo -e "$PASS"
else
    echo -e "$FAIL"
    echo "   → Start frontend: npm run dev"
    ALL_PASS=false
fi

# Check 3: Database exists
echo -n "3. Database file exists... "
DB_PATH="$HOME/.config/claude-view/database.db"
if [ -f "$DB_PATH" ]; then
    echo -e "$PASS"
else
    echo -e "$FAIL"
    echo "   → Database not found at $DB_PATH"
    echo "   → Run: cargo run -p server -- index"
    ALL_PASS=false
fi

# Check 4: Session count
echo -n "4. Session count (need ≥50 for tests)... "
if [ -f "$DB_PATH" ]; then
    SESSION_COUNT=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM sessions;" 2>/dev/null || echo "0")
    if [ "$SESSION_COUNT" -ge 50 ]; then
        echo -e "$PASS (found $SESSION_COUNT sessions)"
    elif [ "$SESSION_COUNT" -gt 0 ]; then
        echo -e "$WARN (found $SESSION_COUNT sessions, recommended: ≥50)"
        echo "   → Some tests may not be representative with low session count"
    else
        echo -e "$FAIL (no sessions found)"
        echo "   → Run: cargo run -p server -- index"
        ALL_PASS=false
    fi
else
    echo -e "SKIP (no database)"
fi

# Check 5: Sessions with branches
echo -n "5. Sessions with git branches... "
if [ -f "$DB_PATH" ]; then
    BRANCH_COUNT=$(sqlite3 "$DB_PATH" "SELECT COUNT(DISTINCT git_branch) FROM sessions WHERE git_branch IS NOT NULL;" 2>/dev/null || echo "0")
    if [ "$BRANCH_COUNT" -ge 3 ]; then
        echo -e "$PASS (found $BRANCH_COUNT branches)"
    elif [ "$BRANCH_COUNT" -gt 0 ]; then
        echo -e "$WARN (found $BRANCH_COUNT branches, recommended: ≥3)"
    else
        echo -e "$WARN (no branches found)"
        echo "   → Phase A and E tests will show empty states"
    fi
else
    echo -e "SKIP (no database)"
fi

# Check 6: Sessions with commits
echo -n "6. Sessions with commits... "
if [ -f "$DB_PATH" ]; then
    COMMIT_COUNT=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM sessions WHERE commit_count > 0;" 2>/dev/null || echo "0")
    if [ "$COMMIT_COUNT" -gt 0 ]; then
        echo -e "$PASS (found $COMMIT_COUNT sessions)"
    else
        echo -e "$WARN (no sessions with commits)"
        echo "   → Phase F tests (git stats) will not be testable"
    fi
else
    echo -e "SKIP (no database)"
fi

# Check 7: Sessions with LOC data
echo -n "7. Sessions with LOC data... "
if [ -f "$DB_PATH" ]; then
    LOC_COUNT=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM sessions WHERE lines_added > 0 OR lines_removed > 0;" 2>/dev/null || echo "0")
    if [ "$LOC_COUNT" -gt 0 ]; then
        echo -e "$PASS (found $LOC_COUNT sessions)"
    else
        echo -e "$WARN (no LOC data found)"
        echo "   → Phase C tests will show empty states"
        echo "   → Run deep indexing to populate LOC data"
    fi
else
    echo -e "SKIP (no database)"
fi

# Check 8: Sessions with files edited
echo -n "8. Sessions with files edited... "
if [ -f "$DB_PATH" ]; then
    FILES_COUNT=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM sessions WHERE files_edited_count > 0;" 2>/dev/null || echo "0")
    if [ "$FILES_COUNT" -gt 0 ]; then
        echo -e "$PASS (found $FILES_COUNT sessions)"
    else
        echo -e "$WARN (no files edited data)"
        echo "   → Phase A test A2 will not be testable"
    fi
else
    echo -e "SKIP (no database)"
fi

# Check 9: Test documentation
echo -n "9. Test documentation exists... "
if [ -f "docs/testing/session-discovery-e2e-tests.md" ]; then
    echo -e "$PASS"
else
    echo -e "$FAIL"
    echo "   → Test plan document not found"
    ALL_PASS=false
fi

# Check 10: Node modules (for frontend)
echo -n "10. Node modules installed... "
if [ -d "node_modules" ]; then
    echo -e "$PASS"
else
    echo -e "$WARN"
    echo "   → Run: npm install"
fi

echo ""
echo "======================================================"

if [ "$ALL_PASS" = true ]; then
    echo -e "${GREEN}✅ Environment ready for E2E testing!${NC}"
    echo ""
    echo "Next steps:"
    echo "  1. Read: docs/testing/TESTING_GUIDE.md"
    echo "  2. Execute: docs/testing/session-discovery-e2e-tests.md"
    echo "  3. Report: Fill out test results template"
    exit 0
else
    echo -e "${RED}❌ Environment not ready. Fix issues above and retry.${NC}"
    echo ""
    echo "Quick fixes:"
    echo "  • Backend:  cargo run -p server"
    echo "  • Frontend: npm run dev"
    echo "  • Index:    cargo run -p server -- index"
    exit 1
fi
