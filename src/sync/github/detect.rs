use anyhow::Result;

/// Auto-detect the GitHub owner/repo from the current directory's git remotes.
///
/// Prefers the `origin` remote. Handles both SSH and HTTPS formats:
/// - `git@github.com:owner/repo.git`
/// - `https://github.com/owner/repo.git`
/// - `https://github.com/owner/repo`
pub fn detect_repo() -> Result<(String, String)> {
    let output = std::process::Command::new("git")
        .args(["remote", "-v"])
        .output()
        .map_err(|_| anyhow::anyhow!("Could not run git. Is git installed?"))?;

    if !output.status.success() {
        anyhow::bail!("Not inside a git repository.");
    }

    let remotes = String::from_utf8_lossy(&output.stdout);
    let mut origin_match = None;
    let mut any_match = None;

    for line in remotes.lines() {
        // Each line: "name\turl (fetch|push)"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let name = parts[0];
        let url = parts[1];

        if let Some(parsed) = parse_github_url(url) {
            if name == "origin" {
                origin_match = Some(parsed);
            } else if any_match.is_none() {
                any_match = Some(parsed);
            }
        }
    }

    origin_match.or(any_match).ok_or_else(|| {
        anyhow::anyhow!(
            "Could not determine GitHub repository from git remotes.\n\n\
             Try one of:\n  \
             vex sync github <owner>/<repo>\n  \
             git remote add origin https://github.com/<owner>/<repo>.git"
        )
    })
}

/// Parse a GitHub URL into (owner, repo).
fn parse_github_url(url: &str) -> Option<(String, String)> {
    // SSH: git@github.com:owner/repo.git
    if let Some(path) = url.strip_prefix("git@github.com:") {
        return parse_owner_repo(path);
    }

    // HTTPS: https://github.com/owner/repo.git
    let prefixes = [
        "https://github.com/",
        "http://github.com/",
        "ssh://git@github.com/",
    ];
    for prefix in prefixes {
        if let Some(path) = url.strip_prefix(prefix) {
            return parse_owner_repo(path);
        }
    }

    None
}

/// Parse "owner/repo.git" or "owner/repo" into (owner, repo).
fn parse_owner_repo(path: &str) -> Option<(String, String)> {
    let path = path.trim_end_matches(".git").trim_end_matches('/');
    let parts: Vec<&str> = path.splitn(2, '/').collect();
    if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
        Some((parts[0].to_string(), parts[1].to_string()))
    } else {
        None
    }
}

/// Parse an explicit "owner/repo" string.
pub fn parse_repo_arg(repo: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = repo.splitn(2, '/').collect();
    if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
        Ok((parts[0].to_string(), parts[1].to_string()))
    } else {
        anyhow::bail!(
            "Invalid repository format: '{repo}'. Expected: owner/repo (e.g., calumjs/vex)"
        )
    }
}
