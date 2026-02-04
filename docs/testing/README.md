# Testing Documentation

This directory contains comprehensive testing documentation for the session discovery features.

## Documents

### [TESTING_GUIDE.md](./TESTING_GUIDE.md)
**Start here!** Complete guide for executing E2E tests, including:
- Environment setup instructions
- Test execution workflows (quick vs. full)
- Playwright browser tool usage examples
- Common issues and solutions
- Best practices and tips

### [session-discovery-e2e-tests.md](./session-discovery-e2e-tests.md)
**Full test specification** with 50+ detailed test cases covering:
- Phase A: Session Card Enhancements (3 tests)
- Phase B: SessionToolbar, Group-by, Filters (9 tests)
- Phase C: LOC Estimation Display (2 tests)
- Phase D: Compact Table View (3 tests)
- Phase E: Sidebar Branch List and Tree View (4 tests)
- Phase F: Git Diff Stats Overlay (2 tests)
- Integration Tests (3 tests)
- Accessibility Tests (5 tests)
- Performance Tests (3 tests)
- Error Handling Tests (3 tests)

Each test includes:
- Objective statement
- Step-by-step instructions
- Expected results
- Acceptance criteria references
- Success/failure conditions

## Quick Start

```bash
# 1. Start services
cargo run -p server &
npm run dev &

# 2. Read the guide
cat docs/testing/TESTING_GUIDE.md

# 3. Execute tests using Playwright browser tool
# Follow instructions in session-discovery-e2e-tests.md

# 4. Document results
# Use template in session-discovery-e2e-tests.md
```

## Test Coverage

| Phase | Feature | Test Count | Priority |
|-------|---------|------------|----------|
| A | Session Card Enhancements | 3 | P0 |
| B | Toolbar + Filters + Grouping | 9 | P0 |
| C | LOC Estimation | 2 | P1 |
| D | Compact Table View | 3 | P1 |
| E | Sidebar Branches + Tree | 4 | P1 |
| F | Git Diff Stats | 2 | P2 |
| Integration | Cross-phase features | 3 | P0 |
| Accessibility | A11y compliance | 5 | P0 |
| Performance | Speed benchmarks | 3 | P1 |
| Error Handling | Graceful failures | 3 | P1 |
| **Total** | | **37** | |

**Additional tests in full spec:** 15+ detailed verification steps = **52 total test cases**

## Test Execution Status

| Run Date | Tester | Tests Passed | Tests Failed | Notes |
|----------|--------|--------------|--------------|-------|
| _Pending_ | — | — | — | Awaiting execution |

**Next step:** Execute tests and fill in this table.

## Related Documentation

- [Design Spec](../plans/2026-02-04-session-discovery-design.md) - Original design document with all acceptance criteria
- [PROGRESS.md](../plans/PROGRESS.md) - Project implementation status
- [Implementation](../../src/components/) - Source code for tested features

## Contributing Test Results

When you complete a test run:

1. Fill out the Test Report Template in `session-discovery-e2e-tests.md`
2. Save screenshots to `./screenshots/` subdirectory
3. Update the status table above
4. Create issues for any failures found
5. Update `PROGRESS.md` if tests reveal implementation gaps

---

**Ready to test?** Start with [TESTING_GUIDE.md](./TESTING_GUIDE.md)!
