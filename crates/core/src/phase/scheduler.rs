//! Classification scheduler types: priority, request, result.

use super::SessionPhase;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    New = 0,
    Transition = 1,
    Steady = 2,
}

pub struct ClassifyResult {
    pub session_id: String,
    pub phase: SessionPhase,
    pub scope: Option<String>,
    /// Generation counter from the request that produced this result.
    /// Used to reject stale in-flight results that arrive after newer ones.
    pub generation: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_ordering() {
        assert!(Priority::New < Priority::Transition);
        assert!(Priority::Transition < Priority::Steady);
    }
}
