# Phase 0 rehearsal — CQRS redesign IRREVERSIBLE drops (migration 63).
#
# Drops 4 dead tables, 7 dead columns on sessions, 1 dead column on models.
# No forward rollback exists — restoration requires restoring from $SNAPSHOT
# and `git checkout pre-phase0`.

phase_forward_migrations() {
  # Apply the CQRS Phase 0 drops against $REHEARSAL_DB. Each DROP COLUMN runs
  # individually so "no such column" on an out-of-band-cleaned DB is tolerated
  # (mirrors lib.rs::run_migrations' same-policy error handling). Mirrors the
  # Rust migration exactly; version number is picked up from the MIGRATIONS
  # array length at code-migration time (not baked into this script).
  local drop_sessions_cols=(
    closed_at dismissed_at session_kind start_type
    prompt_word_count correction_count same_file_edit_count
  )

  sqlite3 "$REHEARSAL_DB" "DROP TABLE IF EXISTS turn_metrics;" || return 1
  sqlite3 "$REHEARSAL_DB" "DROP TABLE IF EXISTS api_errors;" || return 1
  sqlite3 "$REHEARSAL_DB" "DROP TABLE IF EXISTS fluency_scores;" || return 1
  sqlite3 "$REHEARSAL_DB" "DROP TABLE IF EXISTS pricing_cache;" || return 1

  for col in "${drop_sessions_cols[@]}"; do
    # Tolerate "no such column" — column may have been dropped out-of-band.
    sqlite3 "$REHEARSAL_DB" "ALTER TABLE sessions DROP COLUMN $col;" 2>&1 \
      | grep -v "^Parse error near\|^no such column" || true
  done
  sqlite3 "$REHEARSAL_DB" "ALTER TABLE models DROP COLUMN sdk_supported;" 2>&1 \
    | grep -v "^Parse error near\|^no such column" || true
}

phase_smoke_test() {
  # Verify the drops landed on $ACTIVE_DB + the rest of the schema survived.
  # We check via sqlite3 directly rather than booting the full app so the
  # rehearsal stays self-contained and doesn't need a running server.
  local dead_cols_hit
  dead_cols_hit=$(sqlite3 "$ACTIVE_DB" \
    "SELECT name FROM pragma_table_info('sessions') \
     WHERE name IN ('closed_at','dismissed_at','session_kind','start_type', \
                    'prompt_word_count','correction_count','same_file_edit_count');")
  if [ -n "$dead_cols_hit" ]; then
    echo "  FAIL: sessions still has dead columns: $dead_cols_hit"
    return 1
  fi

  local sdk_supported_hit
  sdk_supported_hit=$(sqlite3 "$ACTIVE_DB" \
    "SELECT name FROM pragma_table_info('models') WHERE name = 'sdk_supported';")
  if [ -n "$sdk_supported_hit" ]; then
    echo "  FAIL: models.sdk_supported still present"
    return 1
  fi

  local dead_tbl_hit
  dead_tbl_hit=$(sqlite3 "$ACTIVE_DB" \
    "SELECT name FROM sqlite_master WHERE type = 'table' \
     AND name IN ('turn_metrics','api_errors','fluency_scores','pricing_cache');")
  if [ -n "$dead_tbl_hit" ]; then
    echo "  FAIL: dead tables still present: $dead_tbl_hit"
    return 1
  fi

  # Live schema sanity — sessions and models must still exist with core cols.
  local sessions_cnt models_cnt
  sessions_cnt=$(sqlite3 "$ACTIVE_DB" "SELECT COUNT(*) FROM pragma_table_info('sessions');")
  models_cnt=$(sqlite3 "$ACTIVE_DB" "SELECT COUNT(*) FROM pragma_table_info('models');")
  if [ "$sessions_cnt" -lt 20 ]; then
    echo "  FAIL: sessions table has only $sessions_cnt columns — drop went too far"
    return 1
  fi
  if [ "$models_cnt" -lt 3 ]; then
    echo "  FAIL: models table has only $models_cnt columns — drop went too far"
    return 1
  fi

  echo "  OK: 7 dead sessions cols gone, 1 dead models col gone, 4 dead tables gone"
  echo "      sessions: $sessions_cnt cols, models: $models_cnt cols"
}

phase_rollback_check() {
  # Rollback verification: $ACTIVE_DB is the $SNAPSHOT (pre-migration state,
  # byte-identical to the source prod DB at rehearsal start). We just need
  # proof the snapshot is valid SQLite and has the sessions table (so a
  # rollback cp-overwrite to $PROD_DB would produce a bootable state).
  #
  # We DO NOT require the dead columns/tables to be present in the snapshot:
  # if they were already dropped out-of-band pre-rehearsal, the snapshot is
  # still the exact pre-migration state and thus a valid rollback target.
  local sessions_exists
  sessions_exists=$(sqlite3 "$ACTIVE_DB" \
    "SELECT name FROM sqlite_master WHERE type='table' AND name='sessions';")
  if [ -z "$sessions_exists" ]; then
    echo "  FAIL: snapshot missing sessions table — snapshot is corrupt"
    return 1
  fi

  local sessions_cnt
  sessions_cnt=$(sqlite3 "$ACTIVE_DB" "SELECT COUNT(*) FROM sessions;")
  echo "  OK: snapshot is a valid rollback target ($sessions_cnt sessions, schema intact)"
}
