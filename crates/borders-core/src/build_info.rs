//! Build metadata injected at compile time

/// The version of the application from Cargo.toml
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The git commit hash from .source-commit file or git command
pub const GIT_COMMIT: &str = env!("BUILD_GIT_COMMIT");

/// The build timestamp in RFC3339 format (UTC)
pub const BUILD_TIME: &str = env!("BUILD_TIME");

/// Get the git commit hash (short form, first 7 characters)
pub fn git_commit_short() -> &'static str {
    let full = GIT_COMMIT;
    if full.len() >= 7 { &full[..7] } else { full }
}

/// Full build information formatted as a string
pub fn info() -> String {
    format!("Iron Borders v{} ({})\nBuilt: {}", VERSION, git_commit_short(), BUILD_TIME)
}
