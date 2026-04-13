use anyhow::Result;

/// Resolve a GitHub token from available sources.
///
/// Tries in order:
/// 1. `gh auth token` (GitHub CLI)
/// 2. `GITHUB_TOKEN` environment variable
/// 3. `GH_TOKEN` environment variable
pub fn get_token() -> Result<String> {
    // 1. Try GitHub CLI
    if let Ok(output) = std::process::Command::new("gh")
        .args(["auth", "token"])
        .output()
    {
        if output.status.success() {
            let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !token.is_empty() {
                return Ok(token);
            }
        }
    }

    // 2. Try GITHUB_TOKEN env var
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if !token.is_empty() {
            return Ok(token);
        }
    }

    // 3. Try GH_TOKEN env var
    if let Ok(token) = std::env::var("GH_TOKEN") {
        if !token.is_empty() {
            return Ok(token);
        }
    }

    anyhow::bail!(
        "GitHub authentication required.\n\n\
         Try one of:\n  \
         1. Install gh CLI and run: gh auth login\n  \
         2. Set GITHUB_TOKEN environment variable\n  \
         3. Set GH_TOKEN environment variable"
    )
}
