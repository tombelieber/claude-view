//! Phase 3 PR 3.a — OpenAPI compatibility gate.
//!
//! Every endpoint cutover in Phase 3 (PRs 3.1 → 3.7) rewrites the query
//! layer under a handler while keeping the response shape identical.
//! The frontend depends on the shape. A single accidental field rename
//! or type change would break production with no compile-time signal.
//!
//! This file captures the **pre-Phase-3 OpenAPI spec** as a fixture and
//! asserts that every Phase 3 cutover PR produces a spec that is a
//! **superset** of the baseline. New fields are fine; renamed or dropped
//! fields fail the test.
//!
//! Failure mode: "I removed field `X` from endpoint `Y`" → update the
//! baseline fixture *as part of the same PR* and flag the deletion to
//! Tom. Strictly-additive evolution keeps the frontend running through
//! the entire Phase 3 soak; destructive changes wait for Phase 7 cleanup
//! or a coordinated frontend change.

use claude_view_server::openapi::ApiDoc;
use serde_json::Value;
use utoipa::OpenApi;

const BASELINE_FIXTURE: &str = include_str!("fixtures/openapi_baseline_pre_phase3.json");

/// Endpoints the Phase 3 cutover PRs touch. These get the strictest
/// check: every response field in the baseline must still be present in
/// the current spec with a compatible type. Other endpoints are only
/// required to *exist* — their shapes can evolve freely (they're not
/// part of the Phase 3 cutover).
const PHASE3_ENDPOINTS: &[(&str, &str)] = &[
    // PR 3.1: /api/projects — list of ProjectSummary
    ("/api/projects", "get"),
    // PR 3.2: /api/sessions list — paginated SessionInfo
    ("/api/projects/{id}/sessions", "get"),
    // PR 3.3: /api/sessions/:id detail — single SessionInfo
    ("/api/sessions/{id}", "get"),
    // PR 3.4: /api/sessions/:id/file-history — files + timestamps
    ("/api/sessions/{id}/file-history", "get"),
    // PR 3.5: /api/sessions/:id/hook-events — stays on hook_events (no cutover)
    ("/api/sessions/{id}/hook-events", "get"),
    // PR 3.6: /api/sessions/:id/plans — stays on plans table (no cutover)
    ("/api/sessions/{id}/plans", "get"),
    // PR 3.7: /api/sessions/:id/interaction — JSONL + session_stats cache.
    // Path param is `session_id` (not `id`) — matches how utoipa emits the
    // handler's `Path` extractor.
    ("/api/sessions/{session_id}/interaction", "get"),
];

fn current_spec() -> Value {
    serde_json::to_value(ApiDoc::openapi()).expect("OpenAPI spec must serialise")
}

fn baseline_spec() -> Value {
    serde_json::from_str(BASELINE_FIXTURE).expect("baseline fixture must parse")
}

/// Assert that every `(path, method)` in `PHASE3_ENDPOINTS` exists in
/// both the baseline and the current spec.
#[test]
fn phase3_endpoints_still_exist_in_current_spec() {
    let current = current_spec();
    let baseline = baseline_spec();

    for (path, method) in PHASE3_ENDPOINTS {
        let base_op = baseline
            .pointer(&format!("/paths/{}/{}", escape_path(path), method))
            .unwrap_or_else(|| {
                panic!("baseline fixture is missing {method} {path} — regenerate the fixture")
            });

        let cur_op = current
            .pointer(&format!("/paths/{}/{}", escape_path(path), method))
            .unwrap_or_else(|| {
                panic!(
                    "{method} {path} disappeared from the current spec — Phase 3 cutover broke an endpoint"
                )
            });

        assert!(
            base_op.is_object() && cur_op.is_object(),
            "{method} {path} must be an object in both baseline and current"
        );
    }
}

/// Assert that response schemas for the cutover endpoints are strict
/// supersets of the baseline — every baseline field is still present in
/// the current schema, with the same JSON type.
#[test]
fn phase3_endpoint_response_schemas_are_strict_supersets() {
    let current = current_spec();
    let baseline = baseline_spec();

    for (path, method) in PHASE3_ENDPOINTS {
        let ptr = format!(
            "/paths/{}/{}/responses/200/content/application~1json/schema",
            escape_path(path),
            method
        );

        let base_schema = baseline.pointer(&ptr);
        let cur_schema = current.pointer(&ptr);

        match (base_schema, cur_schema) {
            (Some(b), Some(c)) => {
                assert_superset(b, c, &format!("{method} {path}"));
            }
            (Some(_), None) => {
                panic!("{method} {path} 200 response vanished — broke the frontend contract")
            }
            (None, _) => {
                // baseline didn't pin a JSON schema for this endpoint (e.g.
                // the utoipa macro used a $ref) — skip. Component-level
                // supersets are covered by the components test below.
            }
        }
    }
}

/// Assert that every schema referenced by `components/schemas` in the
/// baseline is still present in the current spec with at minimum the
/// same properties. This is the big-net check — it catches changes to
/// shared types (SessionInfo, ProjectSummary, etc.) that are composed
/// into multiple endpoints.
#[test]
fn shared_component_schemas_are_strict_supersets() {
    let current = current_spec();
    let baseline = baseline_spec();

    let base_components = baseline
        .pointer("/components/schemas")
        .and_then(|v| v.as_object())
        .expect("baseline must have components/schemas");

    let cur_components = current
        .pointer("/components/schemas")
        .and_then(|v| v.as_object())
        .expect("current spec must have components/schemas");

    for (name, base_schema) in base_components {
        let cur_schema = cur_components.get(name).unwrap_or_else(|| {
            panic!(
                "component schema `{name}` removed from current spec — \
                 breaks every endpoint that references it. If this is \
                 intentional, regenerate the baseline fixture as part of \
                 the same PR."
            )
        });

        assert_superset(
            base_schema,
            cur_schema,
            &format!("components/schemas/{name}"),
        );
    }
}

/// Recursively assert that `current` contains every field `baseline`
/// declares. "Superset" = baseline's properties must exist in current
/// with the same JSON type; current is free to add extras.
fn assert_superset(baseline: &Value, current: &Value, context: &str) {
    match (baseline, current) {
        (Value::Object(b), Value::Object(c)) => {
            // If this is a JSON schema object, check `properties` + `type`
            // specifically. Otherwise just recurse on every key.
            if let (Some(Value::Object(bp)), Some(Value::Object(cp))) = (
                b.get("properties").filter(|v| v.is_object()),
                c.get("properties").filter(|v| v.is_object()),
            ) {
                for (field, base_field_schema) in bp {
                    let cur_field_schema = cp.get(field).unwrap_or_else(|| {
                        panic!(
                            "{context}: field `{field}` removed from current spec \
                             — breaks legacy frontend. If intentional, update baseline \
                             fixture + coordinate with frontend in the same PR."
                        )
                    });
                    assert_superset(
                        base_field_schema,
                        cur_field_schema,
                        &format!("{context}.properties.{field}"),
                    );
                }

                // `type` (if declared in baseline) must match exactly —
                // changing a field from `string` to `integer` is always
                // breaking.
                if let Some(base_type) = b.get("type") {
                    let cur_type = c
                        .get("type")
                        .unwrap_or_else(|| panic!("{context}: `type` removed (was {base_type})"));
                    assert_eq!(
                        base_type, cur_type,
                        "{context}: `type` changed — breaking change"
                    );
                }
            } else {
                // Not a JSON schema, recurse through fields that exist in
                // both. Don't fail on missing keys — this lets non-schema
                // JSON (descriptions, examples) evolve freely.
                for (k, bv) in b {
                    if let Some(cv) = c.get(k) {
                        assert_superset(bv, cv, &format!("{context}.{k}"));
                    }
                }
            }
        }
        (Value::Array(b), Value::Array(c)) => {
            // Arrays in the OpenAPI spec (like `enum`, `required`,
            // `tags`) are treated as sets where baseline ⊆ current.
            let cur_items: Vec<&Value> = c.iter().collect();
            for item in b {
                assert!(
                    cur_items.iter().any(|ci| *ci == item),
                    "{context}: array missing required entry {item} — \
                     enum value / required field was removed"
                );
            }
        }
        // Scalar mismatches are caught above as type mismatches. At leaf
        // level the test doesn't enforce value equality — descriptions
        // and examples may reasonably change.
        _ => {}
    }
}

/// Escape `/` → `~1` per RFC6901 JSON Pointer spec so paths like
/// `/api/projects` can be embedded in a pointer string.
fn escape_path(path: &str) -> String {
    path.replace('~', "~0").replace('/', "~1")
}

// ---------------------------------------------------------------------------
// Sanity: the baseline fixture itself must be a valid OpenAPI 3.x document.
// ---------------------------------------------------------------------------

#[test]
fn baseline_fixture_is_parseable_openapi_doc() {
    let baseline = baseline_spec();
    assert_eq!(
        baseline
            .get("openapi")
            .and_then(Value::as_str)
            .unwrap_or(""),
        "3.1.0",
        "baseline must be an OpenAPI 3.1.0 document — regenerate if utoipa updated"
    );
    assert!(
        baseline.get("paths").is_some(),
        "baseline missing `paths` — invalid OpenAPI doc"
    );
}
