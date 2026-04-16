//! Feature plugin system for the JSONL-first architecture.
//!
//! Each "feature" is a self-contained module that:
//! - Subscribes to session lifecycle events from the core.
//! - Owns its own storage (SQLite tables, tantivy index, etc).
//! - Can be enabled/disabled at runtime via config.
//! - Has explicit init/shutdown lifecycle hooks.
//!
//! The core dispatches `SessionEvent`s to all enabled features via
//! the `FeatureRegistry`. A broken feature does not take down the
//! others — dispatch collects errors and continues.
//!
//! See `docs/plans/2026-04-16-hardcut-jsonl-first-design.md` for
//! the architectural context.

use std::path::PathBuf;

use crate::session_catalog::CatalogRow;

/// Shared context handed to every feature during `init`. Provides
/// the feature with the paths and handles it needs to do its work.
#[derive(Debug, Clone)]
pub struct FeatureContext {
    pub data_dir: PathBuf,
    pub catalog: crate::session_catalog::SessionCatalog,
}

/// Lifecycle event emitted by the core whenever the session catalog
/// changes. Every enabled feature receives every event.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    Added(CatalogRow),
    Updated(CatalogRow),
    Removed(String),
}

/// Per-feature enable/disable toggles loaded from
/// `~/.claude-view/config.toml` `[features]` section.
#[derive(Debug, Clone, Default)]
pub struct FeatureConfig {
    pub toggles: std::collections::HashMap<String, bool>,
}

impl FeatureConfig {
    pub fn is_enabled(&self, name: &str) -> bool {
        self.toggles.get(name).copied().unwrap_or(false)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FeatureError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("feature disabled: {0}")]
    Disabled(String),
    #[error("feature init failed: {0}")]
    Init(String),
    #[error("other: {0}")]
    Other(String),
}

/// Every feature implements this trait. The core never sees a
/// concrete feature — it only sees `Box<dyn Feature>` instances
/// held by the registry.
pub trait Feature: Send + Sync {
    fn name(&self) -> &'static str;

    fn is_enabled(&self, cfg: &FeatureConfig) -> bool {
        cfg.is_enabled(self.name())
    }

    fn init(&self, _ctx: &FeatureContext) -> Result<(), FeatureError> {
        Ok(())
    }

    fn on_event(&self, _event: &SessionEvent) -> Result<(), FeatureError> {
        Ok(())
    }

    fn shutdown(&self) -> Result<(), FeatureError> {
        Ok(())
    }
}

/// Holds every feature that was enabled + initialised at startup.
pub struct FeatureRegistry {
    features: Vec<Box<dyn Feature>>,
}

impl Default for FeatureRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FeatureRegistry {
    pub fn new() -> Self {
        Self {
            features: Vec::new(),
        }
    }

    pub fn register(
        &mut self,
        feature: Box<dyn Feature>,
        cfg: &FeatureConfig,
        ctx: &FeatureContext,
    ) -> Result<bool, FeatureError> {
        if !feature.is_enabled(cfg) {
            return Ok(false);
        }
        feature.init(ctx)?;
        self.features.push(feature);
        Ok(true)
    }

    pub fn len(&self) -> usize {
        self.features.len()
    }

    pub fn is_empty(&self) -> bool {
        self.features.is_empty()
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.features.iter().map(|f| f.name()).collect()
    }

    pub fn dispatch(&self, event: &SessionEvent) -> Vec<(String, FeatureError)> {
        let mut errors = Vec::new();
        for feat in &self.features {
            if let Err(e) = feat.on_event(event) {
                errors.push((feat.name().to_string(), e));
            }
        }
        errors
    }

    pub fn shutdown_all(&self) -> Vec<(String, FeatureError)> {
        let mut errors = Vec::new();
        for feat in &self.features {
            if let Err(e) = feat.shutdown() {
                errors.push((feat.name().to_string(), e));
            }
        }
        errors
    }
}

/// Lifecycle wrapper for the existing `hook_events` table. Hook
/// events arrive through their own HTTP endpoint, not session catalog
/// events. This Feature impl exists solely to fit hook_events into
/// the runtime-toggle lifecycle (so users can disable it in config
/// if they don't need hook tracking).
pub struct HookEventsFeature;

impl Feature for HookEventsFeature {
    fn name(&self) -> &'static str {
        "hook-events"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct CountingFeature {
        name: &'static str,
        calls: Arc<AtomicUsize>,
    }

    impl Feature for CountingFeature {
        fn name(&self) -> &'static str {
            self.name
        }
        fn on_event(&self, _event: &SessionEvent) -> Result<(), FeatureError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }

    #[test]
    fn registry_respects_config_toggle() {
        let ctx = FeatureContext {
            data_dir: PathBuf::from("/tmp/not-used"),
            catalog: crate::session_catalog::SessionCatalog::new(),
        };
        let mut cfg = FeatureConfig::default();
        cfg.toggles.insert("feat-on".into(), true);
        cfg.toggles.insert("feat-off".into(), false);

        let mut reg = FeatureRegistry::new();

        let on = reg
            .register(
                Box::new(CountingFeature {
                    name: "feat-on",
                    calls: Arc::new(AtomicUsize::new(0)),
                }),
                &cfg,
                &ctx,
            )
            .unwrap();
        let off = reg
            .register(
                Box::new(CountingFeature {
                    name: "feat-off",
                    calls: Arc::new(AtomicUsize::new(0)),
                }),
                &cfg,
                &ctx,
            )
            .unwrap();

        assert!(on);
        assert!(!off);
        assert_eq!(reg.len(), 1);
        assert_eq!(reg.names(), vec!["feat-on"]);
    }

    #[test]
    fn registry_dispatches_to_all_enabled() {
        let ctx = FeatureContext {
            data_dir: PathBuf::from("/tmp/not-used"),
            catalog: crate::session_catalog::SessionCatalog::new(),
        };
        let mut cfg = FeatureConfig::default();
        cfg.toggles.insert("a".into(), true);
        cfg.toggles.insert("b".into(), true);

        let mut reg = FeatureRegistry::new();
        let calls_a = Arc::new(AtomicUsize::new(0));
        let calls_b = Arc::new(AtomicUsize::new(0));
        reg.register(
            Box::new(CountingFeature {
                name: "a",
                calls: calls_a.clone(),
            }),
            &cfg,
            &ctx,
        )
        .unwrap();
        reg.register(
            Box::new(CountingFeature {
                name: "b",
                calls: calls_b.clone(),
            }),
            &cfg,
            &ctx,
        )
        .unwrap();

        let evt = SessionEvent::Removed("sess-x".into());
        for _ in 0..5 {
            let errs = reg.dispatch(&evt);
            assert!(errs.is_empty());
        }
        assert_eq!(calls_a.load(Ordering::Relaxed), 5);
        assert_eq!(calls_b.load(Ordering::Relaxed), 5);
    }
}
