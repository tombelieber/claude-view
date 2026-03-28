use serde::{Deserialize, Serialize};

/// Value that only goes up within a session.
/// None from sender = "didn't send" = no change.
/// Inner field is PRIVATE -- can't bypass .merge().
/// NOTE: Does not derive TS — ts-rs cannot resolve generic `T` in container-level
/// `#[ts(as)]`. Use `#[ts(as = "Option<ConcreteType>")]` at each usage site.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Monotonic<T>(Option<T>);

impl<T> Default for Monotonic<T> {
    fn default() -> Self {
        Self(None)
    }
}

impl<T: PartialOrd> Monotonic<T> {
    pub const fn new() -> Self {
        Self(None)
    }

    #[inline]
    pub fn merge(&mut self, incoming: Option<T>) {
        if let Some(v) = incoming {
            // Guard: reject if v is not comparable (NaN for floats).
            // PartialOrd::partial_cmp returns None for NaN, so we skip.
            if v.partial_cmp(&v) == Some(std::cmp::Ordering::Equal) {
                match &self.0 {
                    Some(c) if *c >= v => {} // stale or equal -- keep current
                    _ => self.0 = Some(v),   // new or higher -- accept
                }
            }
        }
        // None -> no-op (sender didn't include this field)
    }

    pub fn is_none(&self) -> bool {
        self.0.is_none()
    }

    pub fn get(&self) -> Option<&T> {
        self.0.as_ref()
    }

    pub fn into_inner(self) -> Option<T> {
        self.0
    }
}

/// Latest non-null value wins.
/// None from sender = "didn't send" = no change.
/// NOTE: No TS derive — see Monotonic for rationale.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Latest<T>(Option<T>);

impl<T> Default for Latest<T> {
    fn default() -> Self {
        Self(None)
    }
}

impl<T> Latest<T> {
    pub const fn new() -> Self {
        Self(None)
    }

    #[inline]
    pub fn merge(&mut self, incoming: Option<T>) {
        if incoming.is_some() {
            self.0 = incoming;
        }
    }

    pub fn is_none(&self) -> bool {
        self.0.is_none()
    }

    pub fn get(&self) -> Option<&T> {
        self.0.as_ref()
    }

    pub fn into_inner(self) -> Option<T> {
        self.0
    }
}

/// Absence = intentionally cleared (e.g., exited vim mode).
/// None from sender = "this state ended".
/// NOTE: No TS derive — see Monotonic for rationale.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Transient<T>(Option<T>);

impl<T> Default for Transient<T> {
    fn default() -> Self {
        Self(None)
    }
}

impl<T> Transient<T> {
    pub const fn new() -> Self {
        Self(None)
    }

    #[inline]
    pub fn merge(&mut self, incoming: Option<T>) {
        self.0 = incoming; // always overwrite -- None = cleared
    }

    pub fn is_none(&self) -> bool {
        self.0.is_none()
    }

    pub fn get(&self) -> Option<&T> {
        self.0.as_ref()
    }

    pub fn into_inner(self) -> Option<T> {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ts_rs::TS;

    // -- ts-rs flatten spike --

    /// Verifies #[serde(flatten)] + #[ts(flatten)] on a sub-struct
    /// containing Monotonic/Latest/Transient produces flat TypeScript output.
    /// Per-field #[ts(as)] required because generic newtypes can't derive TS.
    #[derive(Debug, Clone, Default, Serialize, TS)]
    #[serde(rename_all = "camelCase")]
    struct SpikeSubStruct {
        #[ts(as = "Option<u64>")]
        #[serde(default, skip_serializing_if = "Monotonic::is_none")]
        pub spike_duration: Monotonic<u64>,
        #[ts(as = "Option<String>")]
        #[serde(default, skip_serializing_if = "Latest::is_none")]
        pub spike_version: Latest<String>,
        #[ts(as = "Option<String>")]
        #[serde(default, skip_serializing_if = "Transient::is_none")]
        pub spike_vim: Transient<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, TS)]
    #[serde(rename_all = "camelCase")]
    struct SpikeParent {
        pub id: String,
        #[serde(flatten)]
        #[ts(flatten)]
        pub sub: SpikeSubStruct,
    }

    #[test]
    fn ts_rs_flatten_spike_produces_flat_fields() {
        // Verify the TypeScript type has flat fields, not a nested `sub` object
        let ts = <SpikeParent as TS>::inline(&ts_rs::Config::default());
        // SpikeParent should contain `spikeDuration`, `spikeVersion`, `spikeVim`
        // as flat fields (not nested under `sub`)
        assert!(
            ts.contains("spikeDuration"),
            "Expected flat field 'spikeDuration' in TS output, got: {ts}"
        );
        assert!(
            ts.contains("spikeVersion"),
            "Expected flat field 'spikeVersion' in TS output, got: {ts}"
        );
        assert!(
            ts.contains("spikeVim"),
            "Expected flat field 'spikeVim' in TS output, got: {ts}"
        );
        // Should NOT contain `sub:` as a nested field
        assert!(
            !ts.contains("sub:"),
            "Expected NO nested 'sub' field in TS output, got: {ts}"
        );
    }

    // -- Monotonic tests --

    #[test]
    fn monotonic_none_to_some() {
        let mut m = Monotonic::<u64>::new();
        m.merge(Some(100));
        assert_eq!(m.get(), Some(&100));
    }

    #[test]
    fn monotonic_some_to_none_is_noop() {
        let mut m = Monotonic::<u64>::new();
        m.merge(Some(100));
        m.merge(None);
        assert_eq!(m.get(), Some(&100));
    }

    #[test]
    fn monotonic_some_to_lower_is_noop() {
        let mut m = Monotonic::<u64>::new();
        m.merge(Some(100));
        m.merge(Some(50));
        assert_eq!(m.get(), Some(&100));
    }

    #[test]
    fn monotonic_some_to_higher_accepts() {
        let mut m = Monotonic::<u64>::new();
        m.merge(Some(100));
        m.merge(Some(200));
        assert_eq!(m.get(), Some(&200));
    }

    #[test]
    fn monotonic_some_to_equal_is_noop() {
        let mut m = Monotonic::<u64>::new();
        m.merge(Some(100));
        m.merge(Some(100));
        assert_eq!(m.get(), Some(&100));
    }

    #[test]
    fn monotonic_none_to_none_stays_none() {
        let mut m = Monotonic::<u64>::new();
        m.merge(None);
        assert_eq!(m.get(), None);
    }

    #[test]
    fn monotonic_f64_works() {
        let mut m = Monotonic::<f64>::new();
        m.merge(Some(1.23));
        m.merge(Some(0.50));
        assert_eq!(m.get(), Some(&1.23));
        m.merge(Some(2.00));
        assert_eq!(m.get(), Some(&2.00));
    }

    #[test]
    fn monotonic_f64_nan_rejected() {
        let mut m = Monotonic::<f64>::new();
        m.merge(Some(1.0));
        m.merge(Some(f64::NAN));
        assert_eq!(m.get(), Some(&1.0));
    }

    // -- Latest tests --

    #[test]
    fn latest_none_to_some() {
        let mut l = Latest::<String>::new();
        l.merge(Some("hello".into()));
        assert_eq!(l.get(), Some(&"hello".to_string()));
    }

    #[test]
    fn latest_some_to_none_is_noop() {
        let mut l = Latest::<String>::new();
        l.merge(Some("hello".into()));
        l.merge(None);
        assert_eq!(l.get(), Some(&"hello".to_string()));
    }

    #[test]
    fn latest_some_to_some_overwrites() {
        let mut l = Latest::<String>::new();
        l.merge(Some("hello".into()));
        l.merge(Some("world".into()));
        assert_eq!(l.get(), Some(&"world".to_string()));
    }

    // -- Transient tests --

    #[test]
    fn transient_none_to_some() {
        let mut t = Transient::<String>::new();
        t.merge(Some("vim".into()));
        assert_eq!(t.get(), Some(&"vim".to_string()));
    }

    #[test]
    fn transient_some_to_none_clears() {
        let mut t = Transient::<String>::new();
        t.merge(Some("vim".into()));
        t.merge(None);
        assert_eq!(t.get(), None);
    }

    // -- is_none / PartialEq tests --

    #[test]
    fn monotonic_default_is_none() {
        let m = Monotonic::<u64>::default();
        assert!(m.is_none());
    }

    #[test]
    fn latest_default_is_none() {
        let l = Latest::<String>::default();
        assert!(l.is_none());
    }

    #[test]
    fn transient_default_is_none() {
        let t = Transient::<String>::default();
        assert!(t.is_none());
    }

    #[test]
    fn monotonic_is_none_after_merge() {
        let mut m = Monotonic::<u64>::new();
        assert!(m.is_none());
        m.merge(Some(42));
        assert!(!m.is_none());
    }

    #[test]
    fn monotonic_partialeq() {
        let mut a = Monotonic::<u64>::new();
        let mut b = Monotonic::<u64>::new();
        assert_eq!(a, b);
        a.merge(Some(1));
        assert_ne!(a, b);
        b.merge(Some(1));
        assert_eq!(a, b);
    }

    #[test]
    fn latest_partialeq() {
        let mut a = Latest::<String>::new();
        let mut b = Latest::<String>::new();
        assert_eq!(a, b);
        a.merge(Some("x".into()));
        assert_ne!(a, b);
        b.merge(Some("x".into()));
        assert_eq!(a, b);
    }

    #[test]
    fn transient_partialeq() {
        let mut a = Transient::<String>::new();
        let mut b = Transient::<String>::new();
        assert_eq!(a, b);
        a.merge(Some("y".into()));
        assert_ne!(a, b);
        b.merge(Some("y".into()));
        assert_eq!(a, b);
    }

    // -- Serde tests --

    #[test]
    fn monotonic_serializes_as_inner_value() {
        let mut m = Monotonic::<u64>::new();
        m.merge(Some(42));
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(json, "42");

        let empty = Monotonic::<u64>::new();
        let json = serde_json::to_string(&empty).unwrap();
        assert_eq!(json, "null");
    }

    #[test]
    fn monotonic_deserializes_from_value() {
        let m: Monotonic<u64> = serde_json::from_str("42").unwrap();
        assert_eq!(m.get(), Some(&42));

        let m: Monotonic<u64> = serde_json::from_str("null").unwrap();
        assert_eq!(m.get(), None);
    }
}
