use sha2::{Digest, Sha256};
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct ServiceMeta {
    pub service: &'static str,
    pub version: &'static str,
    pub build_sha: &'static str,
    pub host_hash: String,
    pub pid: u32,
}

impl ServiceMeta {
    pub fn new(service: &'static str, version: &'static str, build_sha: &'static str) -> Self {
        Self {
            service,
            version,
            build_sha,
            host_hash: compute_host_hash(),
            pid: std::process::id(),
        }
    }
}

fn compute_host_hash() -> String {
    static CACHED: OnceLock<String> = OnceLock::new();
    CACHED
        .get_or_init(|| {
            let hostname = std::env::var("HOSTNAME")
                .or_else(|_| std::env::var("COMPUTERNAME"))
                .unwrap_or_else(|_| "unknown-host".to_string());
            let digest = Sha256::digest(hostname.as_bytes());
            format!("sha256:{}", &hex::encode(digest)[..12])
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_hash_is_stable_across_calls() {
        let a = compute_host_hash();
        let b = compute_host_hash();
        assert_eq!(a, b);
        assert!(a.starts_with("sha256:"));
        assert_eq!(a.len(), "sha256:".len() + 12);
    }

    #[test]
    fn service_meta_captures_pid() {
        let meta = ServiceMeta::new("test", "0.1.0", "abc");
        assert_eq!(meta.pid, std::process::id());
    }
}
