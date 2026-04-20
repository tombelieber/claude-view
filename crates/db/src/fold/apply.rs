//! Per-action fold rules.
//!
//! Each `fold_*` function takes an `&mut Transaction` and the event,
//! UPSERTs into `session_flags`, and returns whether the event was
//! actually applied (vs skipped under LWW). The caller (batch.rs) owns
//! the surrounding TX, the `applied_seq` advance, and the commit, so
//! a kill-9 between any two events in the batch can only leave
//! `applied_seq` at the pre-batch watermark (atomic TX guarantee).
//!
//! LWW policy (§7.1 design doc):
//!   - classify: apply iff `event.at >= session_flags.classified_at`.
//!     On equal timestamps we APPLY (prefer the later-seq writer so
//!     back-to-back classifier updates commit in order).
//!   - archive / unarchive / dismiss: last-write-wins naturally — the
//!     UPSERT always overwrites. The legacy ordering invariant is
//!     preserved because `applied_seq` consumes events in strictly
//!     increasing `seq` order.

use sqlx::{Sqlite, Transaction};

use super::types::{ActionEvent, ClassifyPayload};
use crate::stage_c::flag_delta::FlagDelta;
use crate::stage_c::outbox::insert_flag_delta_tx;
use crate::DbResult;

/// Dispatch a single action event. Returns (applied, lww_skipped).
///
/// `applied = true`: the fold mutated `session_flags` for this event.
/// `lww_skipped = true`: the event was ignored because a newer one
/// already wrote `session_flags.classified_at`. Only meaningful for
/// classify actions.
///
/// On applied=true, ALSO inserts the corresponding FlagDelta into
/// `stage_c_outbox` within the same TX so Stage C's drainer can later
/// fan out compensating rollup UPDATEs. The TX commits both rows or
/// neither — transactional-outbox crash-safety (§6.2 design doc).
pub(crate) async fn fold_event_tx(
    tx: &mut Transaction<'_, Sqlite>,
    event: &ActionEvent,
) -> DbResult<(bool, bool)> {
    match event.action.as_str() {
        "archive" => {
            fold_archive_tx(tx, event).await?;
            insert_flag_delta_tx(
                &mut **tx,
                &FlagDelta::Archive {
                    session_id: event.session_id.clone(),
                    at_ms: event.at,
                },
            )
            .await?;
            Ok((true, false))
        }
        "unarchive" => {
            fold_unarchive_tx(tx, event).await?;
            insert_flag_delta_tx(
                &mut **tx,
                &FlagDelta::Unarchive {
                    session_id: event.session_id.clone(),
                    at_ms: event.at,
                },
            )
            .await?;
            Ok((true, false))
        }
        "dismiss" => {
            fold_dismiss_tx(tx, event).await?;
            insert_flag_delta_tx(
                &mut **tx,
                &FlagDelta::Dismiss {
                    session_id: event.session_id.clone(),
                    at_ms: event.at,
                },
            )
            .await?;
            Ok((true, false))
        }
        "classify" | "reclassify" => {
            let before_category = read_category_l1(tx, &event.session_id).await?;
            let lww_skipped = fold_classify_tx(tx, event).await?;
            if !lww_skipped {
                let after_category = serde_json::from_str::<ClassifyPayload>(&event.payload)
                    .ok()
                    .map(|p| p.l1);
                insert_flag_delta_tx(
                    &mut **tx,
                    &FlagDelta::Classify {
                        session_id: event.session_id.clone(),
                        before_category_l1: before_category,
                        after_category_l1: after_category,
                        at_ms: event.at,
                    },
                )
                .await?;
            }
            Ok((!lww_skipped, lww_skipped))
        }
        _ => Ok((false, false)),
    }
}

async fn read_category_l1(
    tx: &mut Transaction<'_, Sqlite>,
    session_id: &str,
) -> DbResult<Option<String>> {
    let row: Option<Option<String>> =
        sqlx::query_scalar("SELECT category_l1 FROM session_flags WHERE session_id = ?1")
            .bind(session_id)
            .fetch_optional(&mut **tx)
            .await?;
    Ok(row.flatten())
}

async fn fold_archive_tx(tx: &mut Transaction<'_, Sqlite>, event: &ActionEvent) -> DbResult<()> {
    sqlx::query(
        r#"INSERT INTO session_flags (session_id, archived_at, applied_seq)
           VALUES (?1, ?2, ?3)
           ON CONFLICT(session_id) DO UPDATE SET
             archived_at = excluded.archived_at,
             applied_seq = excluded.applied_seq"#,
    )
    .bind(&event.session_id)
    .bind(event.at)
    .bind(event.seq)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn fold_unarchive_tx(tx: &mut Transaction<'_, Sqlite>, event: &ActionEvent) -> DbResult<()> {
    sqlx::query(
        r#"INSERT INTO session_flags (session_id, archived_at, applied_seq)
           VALUES (?1, NULL, ?2)
           ON CONFLICT(session_id) DO UPDATE SET
             archived_at = NULL,
             applied_seq = excluded.applied_seq"#,
    )
    .bind(&event.session_id)
    .bind(event.seq)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn fold_dismiss_tx(tx: &mut Transaction<'_, Sqlite>, event: &ActionEvent) -> DbResult<()> {
    sqlx::query(
        r#"INSERT INTO session_flags (session_id, dismissed_at, applied_seq)
           VALUES (?1, ?2, ?3)
           ON CONFLICT(session_id) DO UPDATE SET
             dismissed_at = excluded.dismissed_at,
             applied_seq = excluded.applied_seq"#,
    )
    .bind(&event.session_id)
    .bind(event.at)
    .bind(event.seq)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Returns true iff the event was SKIPPED under LWW (stale timestamp).
async fn fold_classify_tx(tx: &mut Transaction<'_, Sqlite>, event: &ActionEvent) -> DbResult<bool> {
    // LWW probe — read the current classified_at (if any) and compare.
    let existing: Option<Option<i64>> =
        sqlx::query_scalar("SELECT classified_at FROM session_flags WHERE session_id = ?1")
            .bind(&event.session_id)
            .fetch_optional(&mut **tx)
            .await?;

    let stored_at = existing.flatten().unwrap_or(0);
    if stored_at > event.at {
        return Ok(true);
    }

    let payload: ClassifyPayload = match serde_json::from_str(&event.payload) {
        Ok(p) => p,
        Err(_) => {
            // Malformed payload: skip rather than poison the fold. PR 5.4
            // parity will flag this as drift; a one-off bad row must not
            // stall the watermark forever.
            return Ok(true);
        }
    };

    sqlx::query(
        r#"INSERT INTO session_flags
             (session_id, category_l1, category_l2, category_l3,
              category_confidence, category_source, classified_at,
              applied_seq)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
           ON CONFLICT(session_id) DO UPDATE SET
             category_l1         = excluded.category_l1,
             category_l2         = excluded.category_l2,
             category_l3         = excluded.category_l3,
             category_confidence = excluded.category_confidence,
             category_source     = excluded.category_source,
             classified_at       = excluded.classified_at,
             applied_seq         = excluded.applied_seq"#,
    )
    .bind(&event.session_id)
    .bind(&payload.l1)
    .bind(&payload.l2)
    .bind(&payload.l3)
    .bind(payload.confidence)
    .bind(&payload.source)
    .bind(event.at)
    .bind(event.seq)
    .execute(&mut **tx)
    .await?;

    Ok(false)
}
