use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // Get the workspace root (two levels up from borders-core)
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();

    // Determine if we're in production mode (CI or release profile)
    let is_production = env::var("CI").is_ok() || env::var("PROFILE").map(|p| p == "release").unwrap_or(false);

    // Read git commit from .source-commit file
    let source_commit_path = workspace_root.join(".source-commit");
    let git_commit = if source_commit_path.exists() {
        match fs::read_to_string(&source_commit_path) {
            Ok(content) => content.trim().to_string(),
            Err(e) if is_production => {
                panic!("Failed to read .source-commit file in production: {}", e);
            }
            Err(_) => "unknown".to_string(),
        }
    } else {
        // Fallback to git command if file doesn't exist (local development)
        let git_result = std::process::Command::new("git").args(["rev-parse", "HEAD"]).current_dir(workspace_root).output().ok().and_then(|output| if output.status.success() { String::from_utf8(output.stdout).ok() } else { None }).map(|s| s.trim().to_string());

        match git_result {
            Some(commit) => commit,
            None if is_production => {
                panic!("Failed to acquire git commit in production and .source-commit file does not exist");
            }
            None => "unknown".to_string(),
        }
    };

    // Determine build time based on environment
    let build_time = if let Ok(epoch) = env::var("SOURCE_DATE_EPOCH") {
        // Use provided timestamp for reproducible builds
        match epoch.parse::<i64>().ok().and_then(|ts| chrono::DateTime::from_timestamp(ts, 0)).map(|dt| dt.to_rfc3339()) {
            Some(time) => time,
            None if is_production => {
                panic!("Failed to parse SOURCE_DATE_EPOCH in production: {}", epoch);
            }
            None => "unknown".to_string(),
        }
    } else if env::var("CI").is_ok() {
        // Generate fresh timestamp in CI
        chrono::Utc::now().to_rfc3339()
    } else {
        // Static value for local development
        "dev".to_string()
    };

    // Set environment variables for compile-time access
    println!("cargo:rustc-env=BUILD_GIT_COMMIT={}", git_commit);
    println!("cargo:rustc-env=BUILD_TIME={}", build_time);

    // Only re-run the build script when specific files change
    println!("cargo:rerun-if-changed=build.rs");

    // In CI, watch the .source-commit file if it exists
    if source_commit_path.exists() {
        println!("cargo:rerun-if-changed={}", source_commit_path.display());
    }

    // In local development, watch .git/HEAD to detect branch switches
    // We intentionally don't watch the branch ref file to avoid spurious rebuilds
    if env::var("CI").is_err() {
        let git_head = workspace_root.join(".git").join("HEAD");
        if git_head.exists() {
            println!("cargo:rerun-if-changed={}", git_head.display());
        }
    }
}
