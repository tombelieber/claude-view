/// Sentinel value used in URLs/API to represent "no branch" (NULL git_branch).
/// Tilde is invalid in git branch names, so collision is impossible.
pub const NO_BRANCH_SENTINEL: &str = "~";

/// Type-safe branch filter that eliminates SQL bind index corruption.
#[derive(Debug, Clone, PartialEq)]
pub enum BranchFilter<'a> {
    /// No filter -- return all sessions regardless of branch
    All,
    /// Filter to sessions with this specific branch name
    Named(&'a str),
    /// Filter to sessions with NULL git_branch
    NoBranch,
}

impl<'a> BranchFilter<'a> {
    /// Parse from an optional URL query parameter value.
    /// None -> All, Some("~") -> NoBranch, Some("main") -> Named("main")
    pub fn from_param(param: Option<&'a str>) -> Self {
        match param {
            None | Some("") => BranchFilter::All,
            Some(s) if s == NO_BRANCH_SENTINEL => BranchFilter::NoBranch,
            Some(s) => BranchFilter::Named(s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_param_none() {
        assert_eq!(BranchFilter::from_param(None), BranchFilter::All);
    }

    #[test]
    fn test_from_param_empty() {
        assert_eq!(BranchFilter::from_param(Some("")), BranchFilter::All);
    }

    #[test]
    fn test_from_param_tilde() {
        assert_eq!(BranchFilter::from_param(Some("~")), BranchFilter::NoBranch);
    }

    #[test]
    fn test_from_param_named() {
        assert_eq!(
            BranchFilter::from_param(Some("main")),
            BranchFilter::Named("main")
        );
    }

    #[test]
    fn test_from_param_named_with_slash() {
        assert_eq!(
            BranchFilter::from_param(Some("feature/auth")),
            BranchFilter::Named("feature/auth")
        );
    }
}
