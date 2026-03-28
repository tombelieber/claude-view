use serde::{Deserialize, Serialize};

/// Value that only goes up within a session.
/// None from sender = "didn't send" = no change.
/// Inner field is PRIVATE -- can't bypass .merge().
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Monotonic<T>(Option<T>);

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

    pub fn get(&self) -> Option<&T> {
        self.0.as_ref()
    }

    pub fn into_inner(self) -> Option<T> {
        self.0
    }
}

/// Latest non-null value wins.
/// None from sender = "didn't send" = no change.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Latest<T>(Option<T>);

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

    pub fn get(&self) -> Option<&T> {
        self.0.as_ref()
    }

    pub fn into_inner(self) -> Option<T> {
        self.0
    }
}

/// Absence = intentionally cleared (e.g., exited vim mode).
/// None from sender = "this state ended".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Transient<T>(Option<T>);

impl<T> Transient<T> {
    pub const fn new() -> Self {
        Self(None)
    }

    #[inline]
    pub fn merge(&mut self, incoming: Option<T>) {
        self.0 = incoming; // always overwrite -- None = cleared
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
