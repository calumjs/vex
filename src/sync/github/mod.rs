mod api;
mod auth;
pub(crate) mod detect;
mod render;
mod state;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;

use api::{FetchParams, GithubClient};
use state::{Manifest, ManifestEntry, SyncState};

/// Try to detect the GitHub repo from git remotes. Returns Ok((owner, repo))
/// if successful, Err if not in a git repo or no GitHub remote found.
/// Does not print any error messages.
pub fn detect_repo_silent() -> anyhow::Result<(String, String)> {
    detect::detect_repo()
}

#[derive(Args, Debug)]
pub struct GithubSyncArgs {
    /// Repository in owner/repo format (auto-detected from git remote if omitted)
    pub repo: Option<String>,

    /// What to sync: issues, prs (comma-separated)
    #[arg(long, default_value = "issues,prs")]
    pub include: String,

    /// Filter by state: open, closed, all
    #[arg(long, default_value = "open")]
    pub state: String,

    /// Filter by labels (comma-separated)
    #[arg(long)]
    pub labels: Option<String>,

    /// Maximum number of items to sync
    #[arg(long)]
    pub limit: Option<usize>,

    /// Only sync items updated after this date (ISO 8601) or "last"
    #[arg(long)]
    pub since: Option<String>,

    /// Ignore sync watermark and re-fetch everything
    #[arg(long)]
    pub force: bool,

    /// Comment rendering: inline or none
    #[arg(long, default_value = "inline")]
    pub comments: String,

    /// Custom output directory
    #[arg(long)]
    pub output: Option<PathBuf>,
}

/// Run the GitHub sync pipeline.
pub fn run(args: GithubSyncArgs) -> Result<()> {
    // Step 1: Resolve repository
    let (owner, repo) = if let Some(ref repo_arg) = args.repo {
        detect::parse_repo_arg(repo_arg)?
    } else {
        eprintln!("vex: detecting repository from git remote...");
        detect::detect_repo()?
    };
    eprintln!("vex: syncing {owner}/{repo}");

    // Step 2: Authenticate
    let token = auth::get_token()?;
    eprintln!("vex: authenticated");

    // Step 3: Determine output directory
    let repo_dir = if let Some(ref output) = args.output {
        output.clone()
    } else {
        super::sources_dir()?.join("github").join(&owner).join(&repo)
    };

    // Step 4: Load existing sync state for incremental sync
    let existing_state = state::load_state(&repo_dir);
    let since = if args.force {
        None
    } else if let Some(ref since_arg) = args.since {
        if since_arg == "last" {
            existing_state.as_ref().map(|s| s.last_sync.clone())
        } else {
            Some(since_arg.clone())
        }
    } else {
        existing_state.as_ref().map(|s| s.last_sync.clone())
    };

    if let Some(ref since) = since {
        eprintln!("vex: incremental sync (since {since})");
    } else {
        eprintln!("vex: full sync");
    }

    // Step 5: Parse include kinds
    let kinds: Vec<&str> = args.include.split(',').map(|s| s.trim()).collect();
    let include_issues = kinds.contains(&"issues");
    let include_prs = kinds.contains(&"prs");

    if !include_issues && !include_prs {
        anyhow::bail!("--include must contain at least one of: issues, prs");
    }

    // Step 6: Create output directories
    if include_issues {
        std::fs::create_dir_all(repo_dir.join("issues"))?;
    }
    if include_prs {
        std::fs::create_dir_all(repo_dir.join("prs"))?;
    }

    // Step 7: Fetch from GitHub API
    let agent = super::http_agent();
    let client = GithubClient::new(agent, token, owner.clone(), repo.clone());

    let labels: Vec<String> = args
        .labels
        .as_deref()
        .map(|l| l.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    let params = FetchParams {
        state: args.state.clone(),
        labels,
        since,
        limit: args.limit,
        include_issues,
        include_prs,
    };

    eprintln!("vex: fetching from GitHub API...");
    let items = client
        .fetch_issues(&params)
        .context("Failed to fetch issues from GitHub")?;

    if items.is_empty() {
        eprintln!("vex: no items found");
        return Ok(());
    }

    eprintln!("vex: fetched {} items", items.len());

    // Step 8: Fetch comments and render each item
    let fetch_comments = args.comments == "inline";
    let mut manifest_entries = Vec::new();
    let mut latest_updated = String::new();

    for (i, issue) in items.iter().enumerate() {
        let is_pr = issue.pull_request.is_some();
        let kind = if is_pr { "prs" } else { "issues" };
        let kind_label = if is_pr { "PR" } else { "issue" };

        eprint!(
            "\r  [{}/{}] {} #{}: {}",
            i + 1,
            items.len(),
            kind_label,
            issue.number,
            truncate(&issue.title, 50)
        );

        // Fetch comments
        let comments = if fetch_comments && issue.body.is_some() {
            match client.fetch_comments(issue.number) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("\n  warning: could not fetch comments for #{}: {e}", issue.number);
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        // Render to Markdown
        let markdown = render::render_issue(issue, &comments);

        // Write file
        let slug = render::slugify(&issue.title);
        let filename = format!("{}-{slug}.md", issue.number);
        let filepath = repo_dir.join(kind).join(&filename);
        std::fs::write(&filepath, &markdown)
            .with_context(|| format!("Failed to write {}", filepath.display()))?;

        // Track for manifest
        manifest_entries.push(ManifestEntry {
            number: issue.number,
            kind: kind.to_string(),
            path: format!("{kind}/{filename}"),
            updated_at: issue.updated_at.clone(),
        });

        // Track latest update for watermark
        if issue.updated_at > latest_updated {
            latest_updated = issue.updated_at.clone();
        }
    }

    eprintln!(); // newline after progress

    // Step 9: Save sync state and manifest
    let sync_state = SyncState {
        last_sync: latest_updated,
        repo: format!("{owner}/{repo}"),
        include: kinds.iter().map(|s| s.to_string()).collect(),
        item_count: items.len(),
    };
    state::save_state(&repo_dir, &sync_state)?;

    let manifest = Manifest {
        version: 1,
        source_type: "github".to_string(),
        owner: owner.clone(),
        repo: repo.clone(),
        included_kinds: kinds.iter().map(|s| s.to_string()).collect(),
        synced_at: sync_state.last_sync.clone(),
        files: manifest_entries,
    };
    state::save_manifest(&repo_dir, &manifest)?;

    // Step 10: Print summary
    let display_path = repo_dir.display();
    eprintln!("\nvex: synced {} items to {display_path}", items.len());
    eprintln!("\nSearch with:");
    eprintln!("  vex \"your query\" {display_path}");

    Ok(())
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len())]
    }
}
