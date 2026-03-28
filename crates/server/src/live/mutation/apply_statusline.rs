//! Merge helper functions for statusline fields.
//! Phase 1: called inline from routes/statusline.rs on raw Option<T> fields.
//! Phase 2: replaced by Monotonic<T>::merge() / Latest<T>::merge() on StatuslineFields sub-struct.

/// Monotonic merge for Option<u64>. None = no-op. Only accept if >= current.
/// Called inline from routes/statusline.rs to fix the 7 blind-overwrite fields.
/// Phase 2 will replace with Monotonic<T>::merge() on StatuslineFields sub-struct.
#[inline]
pub fn merge_monotonic_u64(current: &mut Option<u64>, incoming: Option<u64>) {
    if let Some(v) = incoming {
        match *current {
            Some(c) if c >= v => {} // stale — keep current
            _ => *current = Some(v), // new or higher — accept
        }
    }
}

/// Latest merge for Option<f32>. None = no-op. Some = overwrite.
#[inline]
pub fn merge_latest_f32(current: &mut Option<f32>, incoming: Option<f32>) {
    if incoming.is_some() {
        *current = incoming;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- merge_monotonic_u64 tests ---

    #[test]
    fn monotonic_u64_none_preserved() {
        let mut v = Some(17000u64);
        merge_monotonic_u64(&mut v, None);
        assert_eq!(v, Some(17000)); // None = no-op
    }

    #[test]
    fn monotonic_u64_higher_accepted() {
        let mut v = Some(12000u64);
        merge_monotonic_u64(&mut v, Some(17000));
        assert_eq!(v, Some(17000));
    }

    #[test]
    fn monotonic_u64_lower_rejected() {
        let mut v = Some(17000u64);
        merge_monotonic_u64(&mut v, Some(12000));
        assert_eq!(v, Some(17000));
    }

    #[test]
    fn monotonic_u64_none_to_some() {
        let mut v: Option<u64> = None;
        merge_monotonic_u64(&mut v, Some(5000));
        assert_eq!(v, Some(5000));
    }

    // --- merge_latest_f32 tests ---

    #[test]
    fn latest_f32_none_preserved() {
        let mut v = Some(0.85f32);
        merge_latest_f32(&mut v, None);
        assert_eq!(v, Some(0.85));
    }

    #[test]
    fn latest_f32_some_overwrites() {
        let mut v = Some(0.85f32);
        merge_latest_f32(&mut v, Some(0.42));
        assert_eq!(v, Some(0.42)); // can go down — this is "latest", not monotonic
    }

    #[test]
    fn latest_f32_none_to_some() {
        let mut v: Option<f32> = None;
        merge_latest_f32(&mut v, Some(0.73));
        assert_eq!(v, Some(0.73));
    }
}
