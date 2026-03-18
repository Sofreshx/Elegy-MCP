use elegy_descriptor::{Diagnostic, HttpResource};
use elegy_policy::{validate_http_target, HttpPolicy, PolicyViolation};
use std::fmt;
use std::io::Read;
use std::time::Duration;
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpResolvedResource {
    pub id: String,
    pub uri: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub mime_type: String,
    pub descriptor_path: String,
    pub base_url: String,
    pub path: String,
    pub method: String,
    pub max_size_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpAdapterReadResult {
    pub target_url: String,
    pub status_code: u16,
    pub content_type: Option<String>,
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HttpReadError {
    InvalidTargetUrl {
        uri: String,
        base_url: String,
        path: String,
    },
    PolicyDenied {
        uri: String,
        target: String,
        message: String,
    },
    Timeout {
        uri: String,
        target: String,
    },
    Transport {
        uri: String,
        target: String,
        message: String,
    },
    RedirectDenied {
        uri: String,
        target: String,
        status_code: u16,
        location: Option<String>,
    },
    UpstreamStatus {
        uri: String,
        target: String,
        status_code: u16,
    },
    ResponseTooLarge {
        uri: String,
        target: String,
        limit_bytes: u64,
    },
}

impl HttpReadError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidTargetUrl { .. } => "RUNTIME-HTTP-001",
            Self::PolicyDenied { .. } => "RUNTIME-HTTP-002",
            Self::Timeout { .. } => "RUNTIME-HTTP-003",
            Self::Transport { .. } => "RUNTIME-HTTP-004",
            Self::RedirectDenied { .. } => "RUNTIME-HTTP-005",
            Self::UpstreamStatus { .. } => "RUNTIME-HTTP-006",
            Self::ResponseTooLarge { .. } => "RUNTIME-HTTP-007",
        }
    }
}

impl fmt::Display for HttpReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTargetUrl {
                uri,
                base_url,
                path,
            } => write!(
                f,
                "{}: could not form HTTP target for resource URI {uri:?} from base_url {base_url:?} and path {path:?}",
                self.code()
            ),
            Self::PolicyDenied {
                uri,
                target,
                message,
            } => write!(
                f,
                "{}: HTTP target {target:?} for resource URI {uri:?} is outside policy: {message}",
                self.code()
            ),
            Self::Timeout { uri, target } => write!(
                f,
                "{}: HTTP request for resource URI {uri:?} timed out while reading {target}",
                self.code()
            ),
            Self::Transport {
                uri,
                target,
                message,
            } => write!(
                f,
                "{}: HTTP request for resource URI {uri:?} failed for {target}: {message}",
                self.code()
            ),
            Self::RedirectDenied {
                uri,
                target,
                status_code,
                ..
            } => write!(
                f,
                "{}: HTTP request for resource URI {uri:?} refused redirect response {status_code} from {target}",
                self.code()
            ),
            Self::UpstreamStatus {
                uri,
                target,
                status_code,
            } => write!(
                f,
                "{}: HTTP request for resource URI {uri:?} returned upstream status {status_code} from {target}",
                self.code()
            ),
            Self::ResponseTooLarge {
                uri,
                target,
                limit_bytes,
            } => write!(
                f,
                "{}: HTTP response for resource URI {uri:?} from {target} exceeded configured limit {limit_bytes} bytes",
                self.code()
            ),
        }
    }
}

impl std::error::Error for HttpReadError {}

pub struct HttpRequest {
    pub target: Url,
    pub timeout_ms: u64,
}

pub struct HttpResponse {
    pub status_code: u16,
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
    pub location: Option<String>,
    pub body: Box<dyn Read + Send>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HttpClientError {
    Timeout,
    Transport(String),
}

pub trait HttpClient {
    fn get(&self, request: &HttpRequest) -> Result<HttpResponse, HttpClientError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ReqwestHttpClient;

impl HttpClient for ReqwestHttpClient {
    fn get(&self, request: &HttpRequest) -> Result<HttpResponse, HttpClientError> {
        let client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| HttpClientError::Transport(error.to_string()))?;

        let response = client
            .get(request.target.clone())
            .timeout(Duration::from_millis(request.timeout_ms))
            .send()
            .map_err(|error| {
                if error.is_timeout() {
                    HttpClientError::Timeout
                } else {
                    HttpClientError::Transport(error.to_string())
                }
            })?;

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let content_length = response.content_length();

        Ok(HttpResponse {
            status_code: response.status().as_u16(),
            content_type,
            content_length,
            location,
            body: Box::new(response),
        })
    }
}

pub fn validate_http_policy(policy: &HttpPolicy) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for target in &policy.allowed_targets {
        match Url::parse(target) {
            Ok(url) => {
                if let Err(error) = validate_http_target(&url, policy) {
                    diagnostics.push(policy_violation_to_diagnostic(
                        error,
                        "elegy.toml",
                        "policy.http.allowed_targets",
                    ));
                }
            }
            Err(error) => diagnostics.push(
                Diagnostic::error(
                    "RUNTIME-POLICY-002",
                    format!("configured HTTP target {target:?} is invalid: {error}"),
                )
                .with_path("elegy.toml")
                .with_field("policy.http.allowed_targets"),
            ),
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub fn compose_http_resource(
    policy: &HttpPolicy,
    resource: &HttpResource,
) -> Result<HttpResolvedResource, Vec<Diagnostic>> {
    let target_url = join_http_target(&resource.base_url, &resource.path).map_err(|error| {
        vec![Diagnostic::error(
            "RUNTIME-HTTP-001",
            format!(
                "could not form HTTP target from base_url {:?} and path {:?}: {error}",
                resource.base_url, resource.path
            ),
        )
        .with_path(resource.descriptor_path.clone())
        .with_field("resources[].path")]
    })?;

    validate_http_target(&target_url, policy).map_err(|error| {
        vec![http_policy_violation_to_diagnostic(
            error,
            &resource.descriptor_path,
            "resources[].base_url",
        )]
    })?;

    Ok(HttpResolvedResource {
        id: resource.id.clone(),
        uri: resource.uri.clone(),
        title: resource.title.clone(),
        description: resource.description.clone(),
        mime_type: "application/octet-stream".to_string(),
        descriptor_path: resource.descriptor_path.clone(),
        base_url: resource.base_url.clone(),
        path: resource.path.clone(),
        method: "GET".to_string(),
        max_size_bytes: policy.max_response_size_bytes,
    })
}

pub fn read_http_resource<C: HttpClient>(
    policy: &HttpPolicy,
    resource: &HttpResolvedResource,
    client: &C,
) -> Result<HttpAdapterReadResult, HttpReadError> {
    let target = join_http_target(&resource.base_url, &resource.path).map_err(|_| {
        HttpReadError::InvalidTargetUrl {
            uri: resource.uri.clone(),
            base_url: resource.base_url.clone(),
            path: resource.path.clone(),
        }
    })?;

    validate_http_target(&target, policy).map_err(|violation| HttpReadError::PolicyDenied {
        uri: resource.uri.clone(),
        target: target.to_string(),
        message: violation.to_string(),
    })?;

    let request = HttpRequest {
        target: target.clone(),
        timeout_ms: policy.timeout_ms,
    };
    let mut response = client.get(&request).map_err(|error| match error {
        HttpClientError::Timeout => HttpReadError::Timeout {
            uri: resource.uri.clone(),
            target: target.to_string(),
        },
        HttpClientError::Transport(message) => HttpReadError::Transport {
            uri: resource.uri.clone(),
            target: target.to_string(),
            message,
        },
    })?;

    if (300..400).contains(&response.status_code) {
        return Err(HttpReadError::RedirectDenied {
            uri: resource.uri.clone(),
            target: target.to_string(),
            status_code: response.status_code,
            location: response.location.clone(),
        });
    }

    if response
        .content_length
        .is_some_and(|length| length > policy.max_response_size_bytes)
    {
        return Err(HttpReadError::ResponseTooLarge {
            uri: resource.uri.clone(),
            target: target.to_string(),
            limit_bytes: policy.max_response_size_bytes,
        });
    }

    if !(200..300).contains(&response.status_code) {
        return Err(HttpReadError::UpstreamStatus {
            uri: resource.uri.clone(),
            target: target.to_string(),
            status_code: response.status_code,
        });
    }

    let bytes = read_bounded_bytes(&mut response.body, policy.max_response_size_bytes).map_err(
        |error| match error {
            BoundedReadError::Io(message) => HttpReadError::Transport {
                uri: resource.uri.clone(),
                target: target.to_string(),
                message,
            },
            BoundedReadError::LimitExceeded { .. } => HttpReadError::ResponseTooLarge {
                uri: resource.uri.clone(),
                target: target.to_string(),
                limit_bytes: policy.max_response_size_bytes,
            },
        },
    )?;

    let content_type = response.content_type.clone();

    Ok(HttpAdapterReadResult {
        target_url: target.to_string(),
        status_code: response.status_code,
        content_type: content_type.clone(),
        mime_type: content_type.unwrap_or_else(|| resource.mime_type.clone()),
        bytes,
    })
}

fn join_http_target(base_url: &str, path: &str) -> Result<Url, url::ParseError> {
    Url::parse(base_url)?.join(path)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum BoundedReadError {
    Io(String),
    LimitExceeded { limit_bytes: u64 },
}

impl fmt::Display for BoundedReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message) => write!(f, "{message}"),
            Self::LimitExceeded { limit_bytes } => {
                write!(f, "response exceeded configured limit {limit_bytes} bytes")
            }
        }
    }
}

fn read_bounded_bytes<R: Read>(
    mut reader: R,
    limit_bytes: u64,
) -> Result<Vec<u8>, BoundedReadError> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8_192];

    loop {
        let read = reader.read(&mut buffer).map_err(|error| {
            BoundedReadError::Io(format!("failed to read bounded bytes: {error}"))
        })?;

        if read == 0 {
            break;
        }

        bytes.extend_from_slice(&buffer[..read]);
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limit_bytes {
            return Err(BoundedReadError::LimitExceeded { limit_bytes });
        }
    }

    Ok(bytes)
}

fn policy_violation_to_diagnostic(
    violation: PolicyViolation,
    path: &str,
    field: &str,
) -> Diagnostic {
    Diagnostic::error("RUNTIME-POLICY-003", violation.to_string())
        .with_path(path.to_string())
        .with_field(field.to_string())
}

fn http_policy_violation_to_diagnostic(
    violation: PolicyViolation,
    path: &str,
    field: &str,
) -> Diagnostic {
    Diagnostic::error("RUNTIME-HTTP-002", violation.to_string())
        .with_path(path.to_string())
        .with_field(field.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::Mutex;

    type StubResponse = (u16, Option<String>, Option<String>, Vec<u8>, Option<u64>);

    struct StaticStubHttpClient {
        response: Option<StubResponse>,
        error: Option<HttpClientError>,
    }

    impl HttpClient for StaticStubHttpClient {
        fn get(&self, _request: &HttpRequest) -> Result<HttpResponse, HttpClientError> {
            if let Some(error) = &self.error {
                return Err(error.clone());
            }

            let (status_code, content_type, location, body, content_length) = self
                .response
                .clone()
                .expect("response or error must be configured");

            Ok(HttpResponse {
                status_code,
                content_type,
                content_length,
                location,
                body: Box::new(Cursor::new(body)),
            })
        }
    }

    struct BodyStubHttpClient {
        response: Mutex<Option<HttpResponse>>,
    }

    impl HttpClient for BodyStubHttpClient {
        fn get(&self, _request: &HttpRequest) -> Result<HttpResponse, HttpClientError> {
            self.response
                .lock()
                .expect("body stub lock should not be poisoned")
                .take()
                .ok_or_else(|| {
                    HttpClientError::Transport("response or error must be configured".to_string())
                })
        }
    }

    struct PanicHttpClient;

    impl HttpClient for PanicHttpClient {
        fn get(&self, _request: &HttpRequest) -> Result<HttpResponse, HttpClientError> {
            panic!("HTTP client should not be invoked");
        }
    }

    struct FailingReader;

    impl Read for FailingReader {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("simulated body read failure"))
        }
    }

    fn resolved_resource(base_url: &str, max_size_bytes: u64) -> HttpResolvedResource {
        HttpResolvedResource {
            id: "status".to_string(),
            uri: "elegy://runtime-test/resource/status".to_string(),
            title: Some("Status".to_string()),
            description: None,
            mime_type: "application/octet-stream".to_string(),
            descriptor_path: "tests/http.toml".to_string(),
            base_url: base_url.to_string(),
            path: "/status".to_string(),
            method: "GET".to_string(),
            max_size_bytes,
        }
    }

    fn policy(
        allowed_targets: &[&str],
        allow_plaintext_http: bool,
        max_response_size_bytes: u64,
    ) -> HttpPolicy {
        HttpPolicy {
            allowed_targets: allowed_targets
                .iter()
                .map(|target| (*target).to_string())
                .collect(),
            allow_plaintext_http,
            timeout_ms: 1_000,
            max_response_size_bytes,
        }
    }

    #[test]
    fn validate_http_policy_rejects_invalid_allowlist_targets() {
        let error = validate_http_policy(&HttpPolicy {
            allowed_targets: vec!["://not-a-url".to_string()],
            ..HttpPolicy::default()
        })
        .expect_err("invalid allowlist target must fail");

        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "RUNTIME-POLICY-002");
        assert_eq!(error[0].location.path.as_deref(), Some("elegy.toml"));
        assert_eq!(
            error[0].location.field.as_deref(),
            Some("policy.http.allowed_targets")
        );
        assert!(error[0]
            .message
            .starts_with("configured HTTP target \"://not-a-url\" is invalid:"));
    }

    #[test]
    fn validate_http_policy_rejects_credential_bearing_allowlist_targets() {
        let error = validate_http_policy(&HttpPolicy {
            allowed_targets: vec!["https://user:pass@api.example.com".to_string()],
            ..HttpPolicy::default()
        })
        .expect_err("credential-bearing allowlist target must fail");

        assert_eq!(
            error,
            vec![Diagnostic::error(
                "RUNTIME-POLICY-003",
                "HTTP target must not contain embedded credentials: https://user:pass@api.example.com",
            )
            .with_path("elegy.toml")
            .with_field("policy.http.allowed_targets")]
        );
    }

    #[test]
    fn compose_http_resource_builds_get_only_bounded_state() {
        let resource = HttpResource {
            id: "status".to_string(),
            uri: "elegy://runtime-test/resource/status".to_string(),
            title: Some("Status".to_string()),
            description: None,
            base_url: "https://api.example.com".to_string(),
            path: "/status".to_string(),
            descriptor_path: "tests/http.toml".to_string(),
        };

        let resolved =
            compose_http_resource(&policy(&["https://api.example.com"], false, 64), &resource)
                .expect("compose HTTP resource");

        assert_eq!(resolved.method, "GET");
        assert_eq!(resolved.mime_type, "application/octet-stream");
        assert_eq!(resolved.max_size_bytes, 64);
    }

    #[test]
    fn http_runtime_success_reads_bounded_get_response() {
        let client = StaticStubHttpClient {
            response: Some((
                200,
                Some("application/json".to_string()),
                None,
                br#"{"ok":true}"#.to_vec(),
                Some(11),
            )),
            error: None,
        };

        let result = read_http_resource(
            &policy(&["https://api.example.com"], false, 64),
            &resolved_resource("https://api.example.com", 64),
            &client,
        )
        .expect("HTTP resource should read successfully");

        assert_eq!(
            result,
            HttpAdapterReadResult {
                target_url: "https://api.example.com/status".to_string(),
                status_code: 200,
                content_type: Some("application/json".to_string()),
                mime_type: "application/json".to_string(),
                bytes: br#"{"ok":true}"#.to_vec(),
            }
        );
    }

    #[test]
    fn http_runtime_denies_targets_outside_policy_before_request() {
        let error = read_http_resource(
            &policy(&["https://api.other.test"], false, 64),
            &resolved_resource("https://api.example.com", 64),
            &PanicHttpClient,
        )
        .expect_err("out-of-policy target must fail closed");

        assert_eq!(
            error,
            HttpReadError::PolicyDenied {
                uri: "elegy://runtime-test/resource/status".to_string(),
                target: "https://api.example.com/status".to_string(),
                message: "HTTP target https://api.example.com/status is not allowed".to_string(),
            }
        );
    }

    #[test]
    fn http_runtime_denies_plaintext_when_policy_disallows_it() {
        let error = read_http_resource(
            &policy(&["http://api.example.com"], false, 64),
            &resolved_resource("http://api.example.com", 64),
            &PanicHttpClient,
        )
        .expect_err("plaintext target must be denied");

        assert_eq!(
            error,
            HttpReadError::PolicyDenied {
                uri: "elegy://runtime-test/resource/status".to_string(),
                target: "http://api.example.com/status".to_string(),
                message: "plaintext HTTP targets are not allowed: http://api.example.com/status"
                    .to_string(),
            }
        );
    }

    #[test]
    fn http_runtime_rejects_oversize_responses() {
        let client = StaticStubHttpClient {
            response: Some((
                200,
                Some("text/plain".to_string()),
                None,
                b"0123456789".to_vec(),
                Some(10),
            )),
            error: None,
        };

        let error = read_http_resource(
            &policy(&["https://api.example.com"], false, 8),
            &resolved_resource("https://api.example.com", 8),
            &client,
        )
        .expect_err("oversize response must fail");

        assert_eq!(
            error,
            HttpReadError::ResponseTooLarge {
                uri: "elegy://runtime-test/resource/status".to_string(),
                target: "https://api.example.com/status".to_string(),
                limit_bytes: 8,
            }
        );
    }

    #[test]
    fn bounded_reader_distinguishes_io_failures_from_limit_exceeded() {
        assert_eq!(
            read_bounded_bytes(Cursor::new(b"0123456789".to_vec()), 8),
            Err(BoundedReadError::LimitExceeded { limit_bytes: 8 })
        );

        assert_eq!(
            read_bounded_bytes(FailingReader, 8),
            Err(BoundedReadError::Io(
                "failed to read bounded bytes: simulated body read failure".to_string()
            ))
        );
    }

    #[test]
    fn http_runtime_maps_body_read_failures_to_transport() {
        let client = BodyStubHttpClient {
            response: Mutex::new(Some(HttpResponse {
                status_code: 200,
                content_type: Some("application/json".to_string()),
                content_length: None,
                location: None,
                body: Box::new(FailingReader),
            })),
        };

        let error = read_http_resource(
            &policy(&["https://api.example.com"], false, 64),
            &resolved_resource("https://api.example.com", 64),
            &client,
        )
        .expect_err("body read failures must be normalized as transport errors");

        assert_eq!(
            error,
            HttpReadError::Transport {
                uri: "elegy://runtime-test/resource/status".to_string(),
                target: "https://api.example.com/status".to_string(),
                message: "failed to read bounded bytes: simulated body read failure".to_string(),
            }
        );
    }

    #[test]
    fn http_runtime_maps_stream_limit_exceeded_to_response_too_large() {
        let client = BodyStubHttpClient {
            response: Mutex::new(Some(HttpResponse {
                status_code: 200,
                content_type: Some("text/plain".to_string()),
                content_length: None,
                location: None,
                body: Box::new(Cursor::new(b"0123456789".to_vec())),
            })),
        };

        let error = read_http_resource(
            &policy(&["https://api.example.com"], false, 8),
            &resolved_resource("https://api.example.com", 8),
            &client,
        )
        .expect_err("streamed responses exceeding the limit must fail");

        assert_eq!(
            error,
            HttpReadError::ResponseTooLarge {
                uri: "elegy://runtime-test/resource/status".to_string(),
                target: "https://api.example.com/status".to_string(),
                limit_bytes: 8,
            }
        );
    }

    #[test]
    fn http_runtime_normalizes_non_success_statuses() {
        let client = StaticStubHttpClient {
            response: Some((
                404,
                Some("application/json".to_string()),
                None,
                br#"{"error":"nope"}"#.to_vec(),
                Some(16),
            )),
            error: None,
        };

        let error = read_http_resource(
            &policy(&["https://api.example.com"], false, 64),
            &resolved_resource("https://api.example.com", 64),
            &client,
        )
        .expect_err("non-success status must fail");

        assert_eq!(
            error,
            HttpReadError::UpstreamStatus {
                uri: "elegy://runtime-test/resource/status".to_string(),
                target: "https://api.example.com/status".to_string(),
                status_code: 404,
            }
        );
    }

    #[test]
    fn http_runtime_refuses_redirects() {
        let client = StaticStubHttpClient {
            response: Some((
                302,
                Some("text/plain".to_string()),
                Some("https://redirect.example.com/next".to_string()),
                Vec::new(),
                Some(0),
            )),
            error: None,
        };

        let error = read_http_resource(
            &policy(&["https://api.example.com"], false, 64),
            &resolved_resource("https://api.example.com", 64),
            &client,
        )
        .expect_err("redirect must be refused");

        assert_eq!(
            error,
            HttpReadError::RedirectDenied {
                uri: "elegy://runtime-test/resource/status".to_string(),
                target: "https://api.example.com/status".to_string(),
                status_code: 302,
                location: Some("https://redirect.example.com/next".to_string()),
            }
        );
    }

    #[test]
    fn http_runtime_normalizes_timeouts() {
        let client = StaticStubHttpClient {
            response: None,
            error: Some(HttpClientError::Timeout),
        };

        let error = read_http_resource(
            &policy(&["https://api.example.com"], false, 64),
            &resolved_resource("https://api.example.com", 64),
            &client,
        )
        .expect_err("timeout must be normalized");

        assert_eq!(
            error,
            HttpReadError::Timeout {
                uri: "elegy://runtime-test/resource/status".to_string(),
                target: "https://api.example.com/status".to_string(),
            }
        );
    }
}
