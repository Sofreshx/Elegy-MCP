use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;
use url::Url;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyConfig {
    pub filesystem: FilesystemPolicy,
    pub http: HttpPolicy,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FilesystemPolicy {
    pub roots: Vec<String>,
    pub max_file_size_bytes: u64,
    pub allow_symlinks: bool,
}

impl Default for FilesystemPolicy {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            max_file_size_bytes: 1_048_576,
            allow_symlinks: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpPolicy {
    pub allowed_targets: Vec<String>,
    pub allow_plaintext_http: bool,
    pub timeout_ms: u64,
    pub max_response_size_bytes: u64,
}

impl Default for HttpPolicy {
    fn default() -> Self {
        Self {
            allowed_targets: Vec::new(),
            allow_plaintext_http: false,
            timeout_ms: 30_000,
            max_response_size_bytes: 1_048_576,
        }
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PolicyViolation {
    #[error("filesystem roots are not configured")]
    FilesystemRootsMissing,
    #[error("filesystem root {candidate} is not allowed")]
    FilesystemRootNotAllowed { candidate: String },
    #[error("plaintext HTTP targets are not allowed: {target}")]
    PlaintextHttpDenied { target: String },
    #[error("HTTP target must not contain embedded credentials: {target}")]
    HttpTargetHasCredentials { target: String },
    #[error("HTTP target {target} is not allowed")]
    HttpTargetNotAllowed { target: String },
    #[error("file size {size_bytes} exceeds configured limit {limit_bytes}")]
    FileTooLarge { size_bytes: u64, limit_bytes: u64 },
}

pub fn validate_http_target(target: &Url, policy: &HttpPolicy) -> Result<(), PolicyViolation> {
    if has_embedded_credentials(target) {
        return Err(PolicyViolation::HttpTargetHasCredentials {
            target: target.as_str().trim_end_matches('/').to_string(),
        });
    }

    if target.scheme() == "http" && !policy.allow_plaintext_http {
        return Err(PolicyViolation::PlaintextHttpDenied {
            target: target.as_str().to_string(),
        });
    }

    let candidate = target.as_str().trim_end_matches('/');
    let allowed = policy
        .allowed_targets
        .iter()
        .filter_map(|allowed_target| Url::parse(allowed_target).ok())
        .map(|allowed_target| {
            if has_embedded_credentials(&allowed_target) {
                Err(PolicyViolation::HttpTargetHasCredentials {
                    target: allowed_target.as_str().trim_end_matches('/').to_string(),
                })
            } else {
                Ok(url_matches_allowlisted_scope(target, &allowed_target))
            }
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .any(|matches| matches);

    if allowed {
        Ok(())
    } else {
        Err(PolicyViolation::HttpTargetNotAllowed {
            target: candidate.to_string(),
        })
    }
}

pub fn validate_filesystem_root(
    candidate_root: &Path,
    allowed_roots: &[PathBuf],
) -> Result<(), PolicyViolation> {
    if allowed_roots.is_empty() {
        return Err(PolicyViolation::FilesystemRootsMissing);
    }

    if allowed_roots
        .iter()
        .any(|allowed_root| path_within(allowed_root, candidate_root))
    {
        Ok(())
    } else {
        Err(PolicyViolation::FilesystemRootNotAllowed {
            candidate: candidate_root.display().to_string(),
        })
    }
}

pub fn validate_file_size(size_bytes: u64, limit_bytes: u64) -> Result<(), PolicyViolation> {
    if size_bytes <= limit_bytes {
        Ok(())
    } else {
        Err(PolicyViolation::FileTooLarge {
            size_bytes,
            limit_bytes,
        })
    }
}

pub fn path_within(parent: &Path, child: &Path) -> bool {
    child == parent || child.starts_with(parent)
}

fn url_matches_allowlisted_scope(target: &Url, allowed_target: &Url) -> bool {
    schemes_match(target, allowed_target)
        && hosts_match(target, allowed_target)
        && ports_match(target, allowed_target)
        && path_matches_allowlisted_scope(target.path(), allowed_target.path())
        && queries_match(target, allowed_target)
        && fragments_match(target, allowed_target)
}

fn schemes_match(target: &Url, allowed_target: &Url) -> bool {
    target.scheme() == allowed_target.scheme()
}

fn hosts_match(target: &Url, allowed_target: &Url) -> bool {
    target.host_str() == allowed_target.host_str()
}

fn ports_match(target: &Url, allowed_target: &Url) -> bool {
    target.port_or_known_default() == allowed_target.port_or_known_default()
}

fn path_matches_allowlisted_scope(target_path: &str, allowed_path: &str) -> bool {
    if allowed_path.is_empty() || allowed_path == "/" {
        return true;
    }

    target_path == allowed_path
        || target_path
            .strip_prefix(allowed_path)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn queries_match(target: &Url, allowed_target: &Url) -> bool {
    allowed_target.query().is_none() || target.query() == allowed_target.query()
}

fn fragments_match(target: &Url, allowed_target: &Url) -> bool {
    allowed_target.fragment().is_none() || target.fragment() == allowed_target.fragment()
}

fn has_embedded_credentials(target: &Url) -> bool {
    !target.username().is_empty() || target.password().is_some()
}

#[cfg(test)]
mod tests {
    use super::{validate_http_target, HttpPolicy, PolicyViolation};
    use url::Url;

    fn policy(allowed_targets: &[&str]) -> HttpPolicy {
        HttpPolicy {
            allowed_targets: allowed_targets
                .iter()
                .map(|target| (*target).to_string())
                .collect(),
            ..HttpPolicy::default()
        }
    }

    #[test]
    fn http_targets_fail_closed_without_allowlist() {
        let target = Url::parse("https://api.example.com/openapi.json").expect("valid test URL");
        let error = validate_http_target(&target, &HttpPolicy::default())
            .expect_err("target should be rejected without allowlist");

        assert_eq!(
            error,
            PolicyViolation::HttpTargetNotAllowed {
                target: "https://api.example.com/openapi.json".to_string(),
            }
        );
    }

    #[test]
    fn http_targets_allow_matching_origin_scope() {
        let target =
            Url::parse("https://api.example.com/openapi/v1/spec.json").expect("valid test URL");
        let policy = policy(&["https://api.example.com"]);

        assert_eq!(validate_http_target(&target, &policy), Ok(()));
    }

    #[test]
    fn http_targets_reject_host_suffix_bypass() {
        let target =
            Url::parse("https://api.example.com.evil.test/openapi.json").expect("valid test URL");
        let policy = policy(&["https://api.example.com"]);

        let error = validate_http_target(&target, &policy)
            .expect_err("target should be rejected by policy");

        assert_eq!(
            error,
            PolicyViolation::HttpTargetNotAllowed {
                target: "https://api.example.com.evil.test/openapi.json".to_string(),
            }
        );
    }

    #[test]
    fn http_targets_allow_path_scoped_descendants_on_segment_boundary() {
        let target =
            Url::parse("https://api.example.com/v1/openapi/spec.json").expect("valid test URL");
        let policy = policy(&["https://api.example.com/v1"]);

        assert_eq!(validate_http_target(&target, &policy), Ok(()));
    }

    #[test]
    fn http_targets_reject_path_prefix_bypass_without_segment_boundary() {
        let target =
            Url::parse("https://api.example.com/v12/openapi.json").expect("valid test URL");
        let policy = policy(&["https://api.example.com/v1"]);

        let error = validate_http_target(&target, &policy)
            .expect_err("target should be rejected by policy");

        assert_eq!(
            error,
            PolicyViolation::HttpTargetNotAllowed {
                target: "https://api.example.com/v12/openapi.json".to_string(),
            }
        );
    }

    #[test]
    fn http_targets_treat_default_ports_as_equivalent() {
        let target =
            Url::parse("https://api.example.com:443/openapi.json").expect("valid test URL");
        let policy = policy(&["https://api.example.com"]);

        assert_eq!(validate_http_target(&target, &policy), Ok(()));
    }

    #[test]
    fn http_targets_reject_credential_bearing_candidate_urls() {
        let target =
            Url::parse("https://user:pass@api.example.com/openapi.json").expect("valid test URL");
        let policy = policy(&["https://api.example.com"]);

        assert_eq!(
            validate_http_target(&target, &policy),
            Err(PolicyViolation::HttpTargetHasCredentials {
                target: "https://user:pass@api.example.com/openapi.json".to_string(),
            })
        );
    }

    #[test]
    fn http_targets_reject_credential_bearing_allowlist_entries() {
        let target = Url::parse("https://api.example.com/openapi.json").expect("valid test URL");
        let policy = policy(&["https://user:pass@api.example.com"]);

        assert_eq!(
            validate_http_target(&target, &policy),
            Err(PolicyViolation::HttpTargetHasCredentials {
                target: "https://user:pass@api.example.com".to_string(),
            })
        );
    }

    #[test]
    fn http_targets_preserve_query_scoped_allowlist_entries() {
        let target =
            Url::parse("https://api.example.com/openapi.json?version=v2").expect("valid test URL");
        let policy = policy(&["https://api.example.com/openapi.json?version=v1"]);

        let error = validate_http_target(&target, &policy)
            .expect_err("target should be rejected by policy");

        assert_eq!(
            error,
            PolicyViolation::HttpTargetNotAllowed {
                target: "https://api.example.com/openapi.json?version=v2".to_string(),
            }
        );
    }
}
