# Stub — real implementation lands with the forward migration.
# Returns non-zero on --apply so rehearsal fails loudly until wired.

phase_forward_migrations() {
  echo "  [STUB] Not yet wired. Add the real migration SQL here."
  return 1
}

phase_smoke_test() {
  echo "  [STUB] Not yet wired. Boot app + assert on \$ACTIVE_DB here."
  return 1
}

phase_rollback_check() {
  echo "  [STUB] Not yet wired. Boot app on restored \$ACTIVE_DB + old code here."
  return 1
}
