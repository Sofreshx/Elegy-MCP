use elegy_descriptor::{Diagnostic, FilesystemResource, StaticResource};
use elegy_policy::{
    validate_file_size, validate_filesystem_root, FilesystemPolicy, PolicyViolation,
};
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FsResolvedStaticResource {
    pub id: String,
    pub uri: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub mime_type: String,
    pub descriptor_path: String,
    pub content: String,
    pub max_size_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FsResolvedFilesystemResource {
    pub id: String,
    pub uri: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub mime_type: String,
    pub descriptor_path: String,
    pub root: String,
    pub path: String,
    pub max_size_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FsReadError {
    AccessDenied { uri: String, message: String },
    InvalidResourceState { uri: String, message: String },
    Io { uri: String, message: String },
}

pub fn resolve_allowed_roots(
    project_root: &Path,
    policy: &FilesystemPolicy,
) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut roots = BTreeSet::new();

    for root in &policy.roots {
        let path = project_root.join(root);
        if !policy.allow_symlinks
            && path.exists()
            && matches!(
                ensure_symlink_policy(project_root, Path::new(root), false),
                Err(SymlinkPolicyError::Denied)
            )
        {
            diagnostics.push(
                Diagnostic::error(
                    "RUNTIME-FS-003",
                    format!("configured filesystem root {root:?} resolves through a symlink"),
                )
                .with_path("elegy.toml")
                .with_field("policy.filesystem.roots"),
            );
            continue;
        }

        match path.canonicalize() {
            Ok(canonical) => {
                roots.insert(canonical);
            }
            Err(error) => diagnostics.push(
                Diagnostic::error(
                    "RUNTIME-POLICY-001",
                    format!("configured filesystem root {root:?} could not be resolved: {error}"),
                )
                .with_path("elegy.toml")
                .with_field("policy.filesystem.roots"),
            ),
        }
    }

    if diagnostics.is_empty() {
        Ok(roots.into_iter().collect())
    } else {
        Err(diagnostics)
    }
}

pub fn try_compose_static_resource(
    resource: &StaticResource,
    policy: &FilesystemPolicy,
) -> Result<FsResolvedStaticResource, Vec<Diagnostic>> {
    let content_size = u64::try_from(resource.content.len()).unwrap_or(u64::MAX);
    validate_file_size(content_size, policy.max_file_size_bytes).map_err(|error| {
        vec![policy_violation_to_diagnostic(
            error,
            &resource.descriptor_path,
            "resources[].content",
        )]
    })?;

    Ok(FsResolvedStaticResource {
        id: resource.id.clone(),
        uri: resource.uri.clone(),
        title: resource.title.clone(),
        description: resource.description.clone(),
        mime_type: resource
            .mime_type
            .clone()
            .unwrap_or_else(|| "text/plain; charset=utf-8".to_string()),
        descriptor_path: resource.descriptor_path.clone(),
        content: resource.content.clone(),
        max_size_bytes: policy.max_file_size_bytes,
    })
}

pub fn compose_filesystem_resource(
    project_root: &Path,
    allowed_roots: &[PathBuf],
    policy: &FilesystemPolicy,
    resource: &FilesystemResource,
) -> Result<FsResolvedFilesystemResource, Vec<Diagnostic>> {
    let root_path = project_root.join(&resource.root);
    let canonical_root = root_path.canonicalize().map_err(|error| {
        vec![Diagnostic::error(
            "RUNTIME-FS-001",
            format!(
                "filesystem root {:?} could not be resolved: {error}",
                resource.root
            ),
        )
        .with_path(resource.descriptor_path.clone())
        .with_field("resources[].root")]
    })?;

    validate_filesystem_root(&canonical_root, allowed_roots).map_err(|error| {
        vec![policy_violation_to_diagnostic(
            error,
            &resource.descriptor_path,
            "resources[].root",
        )]
    })?;

    let candidate_path = canonical_root.join(&resource.path);
    ensure_symlink_policy(
        &canonical_root,
        Path::new(&resource.path),
        policy.allow_symlinks,
    )
    .map_err(|error| {
        vec![Diagnostic::error(
            error.code(),
            format!(
                "filesystem resource path {:?} {}",
                resource.path,
                error.message()
            ),
        )
        .with_path(resource.descriptor_path.clone())
        .with_field("resources[].path")]
    })?;

    let _metadata = fs::symlink_metadata(&candidate_path).map_err(|error| {
        vec![Diagnostic::error(
            "RUNTIME-FS-002",
            format!(
                "filesystem resource path {:?} could not be read: {error}",
                resource.path
            ),
        )
        .with_path(resource.descriptor_path.clone())
        .with_field("resources[].path")]
    })?;

    let canonical_file = candidate_path.canonicalize().map_err(|error| {
        vec![Diagnostic::error(
            "RUNTIME-FS-004",
            format!(
                "filesystem resource path {:?} could not be resolved: {error}",
                resource.path
            ),
        )
        .with_path(resource.descriptor_path.clone())
        .with_field("resources[].path")]
    })?;

    if !canonical_file.starts_with(&canonical_root) {
        return Err(vec![Diagnostic::error(
            "RUNTIME-FS-005",
            format!(
                "filesystem resource path {:?} escapes its configured root",
                resource.path
            ),
        )
        .with_path(resource.descriptor_path.clone())
        .with_field("resources[].path")]);
    }

    let file_metadata = fs::metadata(&canonical_file).map_err(|error| {
        vec![Diagnostic::error(
            "RUNTIME-FS-006",
            format!(
                "filesystem resource path {:?} could not be inspected: {error}",
                resource.path
            ),
        )
        .with_path(resource.descriptor_path.clone())
        .with_field("resources[].path")]
    })?;

    if !file_metadata.is_file() {
        return Err(vec![Diagnostic::error(
            "RUNTIME-FS-007",
            format!("filesystem resource path {:?} is not a file", resource.path),
        )
        .with_path(resource.descriptor_path.clone())
        .with_field("resources[].path")]);
    }

    validate_file_size(file_metadata.len(), policy.max_file_size_bytes).map_err(|error| {
        vec![policy_violation_to_diagnostic(
            error,
            &resource.descriptor_path,
            "resources[].path",
        )]
    })?;

    Ok(FsResolvedFilesystemResource {
        id: resource.id.clone(),
        uri: resource.uri.clone(),
        title: resource.title.clone(),
        description: resource.description.clone(),
        mime_type: resource
            .mime_type
            .clone()
            .unwrap_or_else(|| infer_mime_type(Path::new(&resource.path))),
        descriptor_path: resource.descriptor_path.clone(),
        root: resource.root.clone(),
        path: resource.path.clone(),
        max_size_bytes: policy.max_file_size_bytes,
    })
}

pub fn read_static_resource(resource: &FsResolvedStaticResource) -> Vec<u8> {
    resource.content.as_bytes().to_vec()
}

pub fn read_filesystem_resource(
    project_root: &Path,
    allowed_roots: &[PathBuf],
    policy: &FilesystemPolicy,
    resource: &FsResolvedFilesystemResource,
) -> Result<Vec<u8>, FsReadError> {
    let uri = resource.uri.clone();
    let canonical_root = project_root
        .join(&resource.root)
        .canonicalize()
        .map_err(|error| FsReadError::InvalidResourceState {
            uri: uri.clone(),
            message: format!(
                "filesystem root {:?} could not be resolved: {error}",
                resource.root
            ),
        })?;

    validate_filesystem_root(&canonical_root, allowed_roots).map_err(|error| {
        FsReadError::AccessDenied {
            uri: uri.clone(),
            message: error.to_string(),
        }
    })?;

    let candidate_path = canonical_root.join(&resource.path);
    ensure_symlink_policy(
        &canonical_root,
        Path::new(&resource.path),
        policy.allow_symlinks,
    )
    .map_err(|error| match error {
        SymlinkPolicyError::Io(error) => FsReadError::InvalidResourceState {
            uri: uri.clone(),
            message: format!(
                "filesystem resource path {:?} could not be read: {error}",
                resource.path
            ),
        },
        SymlinkPolicyError::Denied => FsReadError::AccessDenied {
            uri: uri.clone(),
            message: format!(
                "filesystem resource path {:?} resolves through a symlink",
                resource.path
            ),
        },
    })?;

    let _metadata = fs::symlink_metadata(&candidate_path).map_err(|error| {
        FsReadError::InvalidResourceState {
            uri: uri.clone(),
            message: format!(
                "filesystem resource path {:?} could not be read: {error}",
                resource.path
            ),
        }
    })?;

    let uri = resource.uri.clone();
    let canonical_file =
        candidate_path
            .canonicalize()
            .map_err(|error| FsReadError::InvalidResourceState {
                uri: uri.clone(),
                message: format!(
                    "filesystem resource path {:?} could not be resolved: {error}",
                    resource.path
                ),
            })?;

    if !canonical_file.starts_with(&canonical_root) {
        return Err(FsReadError::AccessDenied {
            uri: uri.clone(),
            message: format!(
                "filesystem resource path {:?} escapes its configured root",
                resource.path
            ),
        });
    }

    let file_metadata =
        fs::metadata(&canonical_file).map_err(|error| FsReadError::InvalidResourceState {
            uri: uri.clone(),
            message: format!(
                "filesystem resource path {:?} could not be inspected: {error}",
                resource.path
            ),
        })?;

    if !file_metadata.is_file() {
        return Err(FsReadError::InvalidResourceState {
            uri: uri.clone(),
            message: format!("filesystem resource path {:?} is not a file", resource.path),
        });
    }

    validate_file_size(file_metadata.len(), policy.max_file_size_bytes).map_err(|error| {
        FsReadError::AccessDenied {
            uri: uri.clone(),
            message: error.to_string(),
        }
    })?;

    let file = fs::File::open(&canonical_file).map_err(|error| FsReadError::Io {
        uri: uri.clone(),
        message: format!(
            "filesystem resource path {:?} could not be opened: {error}",
            resource.path
        ),
    })?;

    read_bounded_bytes(file, policy.max_file_size_bytes).map_err(|error| match error {
        BoundedReadError::Io(message) | BoundedReadError::LimitExceeded { message, .. } => {
            FsReadError::AccessDenied {
                uri: uri.clone(),
                message,
            }
        }
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum BoundedReadError {
    Io(String),
    LimitExceeded { limit_bytes: u64, message: String },
}

impl fmt::Display for BoundedReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message) => write!(f, "{message}"),
            Self::LimitExceeded { message, .. } => write!(f, "{message}"),
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
            return Err(BoundedReadError::LimitExceeded {
                limit_bytes,
                message: format!("response exceeded configured limit {limit_bytes} bytes"),
            });
        }
    }

    Ok(bytes)
}

#[derive(Debug)]
enum SymlinkPolicyError {
    Io(std::io::Error),
    Denied,
}

impl SymlinkPolicyError {
    const fn code(&self) -> &'static str {
        match self {
            Self::Io(..) => "RUNTIME-FS-002",
            Self::Denied => "RUNTIME-FS-003",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::Io(error) => format!("could not be read: {error}"),
            Self::Denied => "resolves through a symlink".to_string(),
        }
    }
}

fn ensure_symlink_policy(
    root: &Path,
    resource_path: &Path,
    allow_symlinks: bool,
) -> Result<(), SymlinkPolicyError> {
    if allow_symlinks {
        return Ok(());
    }

    let mut current = root.to_path_buf();
    for component in resource_path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => continue,
            Component::CurDir | Component::ParentDir | Component::Normal(_) => {
                current.push(component.as_os_str())
            }
        }

        let metadata = fs::symlink_metadata(&current).map_err(SymlinkPolicyError::Io)?;
        if metadata.file_type().is_symlink() {
            return Err(SymlinkPolicyError::Denied);
        }
    }

    Ok(())
}

fn infer_mime_type(path: &Path) -> String {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("json") => "application/json".to_string(),
        Some("md") => "text/markdown; charset=utf-8".to_string(),
        Some("txt") => "text/plain; charset=utf-8".to_string(),
        Some("toml") => "application/toml".to_string(),
        Some("yaml") | Some("yml") => "application/yaml".to_string(),
        _ => "application/octet-stream".to_string(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("elegy-adapter-fs-{unique}"));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn filesystem_policy(root: &str, max_file_size_bytes: u64) -> FilesystemPolicy {
        FilesystemPolicy {
            roots: vec![root.to_string()],
            max_file_size_bytes,
            allow_symlinks: false,
        }
    }

    #[test]
    fn compose_static_resource_defaults_text_plain_and_reads_inline_bytes() {
        let resource = StaticResource {
            id: "hello".to_string(),
            uri: "elegy://test/resource/hello".to_string(),
            title: Some("Hello".to_string()),
            description: None,
            mime_type: None,
            content: "hello world".to_string(),
            descriptor_path: "tests/static.toml".to_string(),
        };
        let policy = filesystem_policy("content", 64);

        let resolved =
            try_compose_static_resource(&resource, &policy).expect("compose static resource");

        assert_eq!(resolved.mime_type, "text/plain; charset=utf-8");
        assert_eq!(resolved.max_size_bytes, 64);
        assert_eq!(read_static_resource(&resolved), b"hello world".to_vec());
    }

    #[test]
    fn compose_static_resource_rejects_inline_content_over_limit() {
        let resource = StaticResource {
            id: "hello".to_string(),
            uri: "elegy://test/resource/hello".to_string(),
            title: None,
            description: None,
            mime_type: None,
            content: "hello world".to_string(),
            descriptor_path: "tests/static.toml".to_string(),
        };
        let policy = filesystem_policy("content", 5);

        assert_eq!(
            try_compose_static_resource(&resource, &policy),
            Err(vec![Diagnostic::error(
                "RUNTIME-POLICY-003",
                "file size 11 exceeds configured limit 5",
            )
            .with_path("tests/static.toml")
            .with_field("resources[].content")])
        );
    }

    #[test]
    fn resolve_allowed_roots_rejects_symlinked_root_when_symlinks_are_disabled() {
        let temp_dir = TempDir::new();
        let real_root = temp_dir.path().join("real-root");
        let linked_root = temp_dir.path().join("linked-root");
        fs::create_dir_all(&real_root).expect("create real root");
        create_dir_symlink(&real_root, &linked_root);

        let policy = filesystem_policy("linked-root", 64);

        assert_eq!(
            resolve_allowed_roots(temp_dir.path(), &policy),
            Err(vec![Diagnostic::error(
                "RUNTIME-FS-003",
                "configured filesystem root \"linked-root\" resolves through a symlink",
            )
            .with_path("elegy.toml")
            .with_field("policy.filesystem.roots")])
        );
    }

    #[test]
    fn filesystem_compose_infers_mime_and_read_revalidates_within_root() {
        let temp_dir = TempDir::new();
        let content_root = temp_dir.path().join("content");
        fs::create_dir_all(&content_root).expect("create content root");
        fs::write(content_root.join("status.json"), br#"{"ok":true}"#).expect("write test file");

        let policy = filesystem_policy("content", 64);
        let allowed_roots = resolve_allowed_roots(temp_dir.path(), &policy).expect("resolve roots");
        let resource = FilesystemResource {
            id: "status".to_string(),
            uri: "elegy://test/resource/status".to_string(),
            title: None,
            description: None,
            mime_type: None,
            root: "content".to_string(),
            path: "status.json".to_string(),
            descriptor_path: "tests/fs.toml".to_string(),
        };

        let resolved =
            compose_filesystem_resource(temp_dir.path(), &allowed_roots, &policy, &resource)
                .expect("compose filesystem resource");

        assert_eq!(resolved.mime_type, "application/json");
        assert_eq!(
            read_filesystem_resource(temp_dir.path(), &allowed_roots, &policy, &resolved)
                .expect("read filesystem resource"),
            br#"{"ok":true}"#.to_vec()
        );
    }

    #[test]
    fn filesystem_read_fails_closed_when_file_grows_beyond_limit() {
        let temp_dir = TempDir::new();
        let content_root = temp_dir.path().join("content");
        fs::create_dir_all(&content_root).expect("create content root");
        let file_path = content_root.join("status.txt");
        fs::write(&file_path, b"short").expect("write small file");

        let policy = filesystem_policy("content", 8);
        let allowed_roots = resolve_allowed_roots(temp_dir.path(), &policy).expect("resolve roots");
        let resource = FilesystemResource {
            id: "status".to_string(),
            uri: "elegy://test/resource/status".to_string(),
            title: None,
            description: None,
            mime_type: None,
            root: "content".to_string(),
            path: "status.txt".to_string(),
            descriptor_path: "tests/fs.toml".to_string(),
        };
        let resolved =
            compose_filesystem_resource(temp_dir.path(), &allowed_roots, &policy, &resource)
                .expect("compose filesystem resource");

        fs::write(&file_path, b"0123456789").expect("grow file after composition");

        assert_eq!(
            read_filesystem_resource(temp_dir.path(), &allowed_roots, &policy, &resolved),
            Err(FsReadError::AccessDenied {
                uri: "elegy://test/resource/status".to_string(),
                message: "file size 10 exceeds configured limit 8".to_string(),
            })
        );
    }

    #[cfg(unix)]
    fn create_dir_symlink(target: &Path, link: &Path) {
        std::os::unix::fs::symlink(target, link).expect("create directory symlink");
    }

    #[cfg(windows)]
    fn create_dir_symlink(target: &Path, link: &Path) {
        let command = format!(
            "New-Item -ItemType Junction -Path '{}' -Target '{}' | Out-Null",
            powershell_single_quoted_path(link),
            powershell_single_quoted_path(target)
        );
        let status = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &command])
            .status()
            .expect("launch powershell to create directory junction");
        assert!(status.success(), "create directory junction");
    }

    #[cfg(windows)]
    fn powershell_single_quoted_path(path: &Path) -> String {
        path.display().to_string().replace('\'', "''")
    }

    #[test]
    fn filesystem_compose_rejects_symlinked_parent_directory() {
        let temp_dir = TempDir::new();
        let content_root = temp_dir.path().join("content");
        let real_dir = content_root.join("real");
        let linked_dir = content_root.join("linked");
        fs::create_dir_all(&real_dir).expect("create real directory");
        fs::write(real_dir.join("status.txt"), b"ok").expect("write test file");
        create_dir_symlink(&real_dir, &linked_dir);

        let policy = filesystem_policy("content", 64);
        let allowed_roots = resolve_allowed_roots(temp_dir.path(), &policy).expect("resolve roots");
        let resource = FilesystemResource {
            id: "status".to_string(),
            uri: "elegy://test/resource/status".to_string(),
            title: None,
            description: None,
            mime_type: None,
            root: "content".to_string(),
            path: "linked/status.txt".to_string(),
            descriptor_path: "tests/fs.toml".to_string(),
        };

        assert_eq!(
            compose_filesystem_resource(temp_dir.path(), &allowed_roots, &policy, &resource),
            Err(vec![Diagnostic::error(
                "RUNTIME-FS-003",
                "filesystem resource path \"linked/status.txt\" resolves through a symlink",
            )
            .with_path("tests/fs.toml")
            .with_field("resources[].path")])
        );
    }

    #[test]
    fn filesystem_read_rejects_path_when_parent_becomes_symlink() {
        let temp_dir = TempDir::new();
        let content_root = temp_dir.path().join("content");
        let original_dir = content_root.join("nested");
        let swapped_dir = content_root.join("swapped");
        fs::create_dir_all(&original_dir).expect("create original directory");
        fs::create_dir_all(&swapped_dir).expect("create replacement directory");
        fs::write(original_dir.join("status.txt"), b"ok").expect("write original file");
        fs::write(swapped_dir.join("status.txt"), b"ok").expect("write replacement file");

        let policy = filesystem_policy("content", 64);
        let allowed_roots = resolve_allowed_roots(temp_dir.path(), &policy).expect("resolve roots");
        let resource = FilesystemResource {
            id: "status".to_string(),
            uri: "elegy://test/resource/status".to_string(),
            title: None,
            description: None,
            mime_type: None,
            root: "content".to_string(),
            path: "nested/status.txt".to_string(),
            descriptor_path: "tests/fs.toml".to_string(),
        };
        let resolved =
            compose_filesystem_resource(temp_dir.path(), &allowed_roots, &policy, &resource)
                .expect("compose filesystem resource");

        fs::remove_file(original_dir.join("status.txt")).expect("remove original file");
        fs::remove_dir(&original_dir).expect("remove original directory");
        create_dir_symlink(&swapped_dir, &original_dir);

        assert_eq!(
            read_filesystem_resource(temp_dir.path(), &allowed_roots, &policy, &resolved),
            Err(FsReadError::AccessDenied {
                uri: "elegy://test/resource/status".to_string(),
                message:
                    "filesystem resource path \"nested/status.txt\" resolves through a symlink"
                        .to_string(),
            })
        );
    }
}
