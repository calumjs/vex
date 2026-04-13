use anyhow::{Context, Result};
use serde::Deserialize;

/// GitHub issue from the REST API.
#[derive(Deserialize, Debug)]
pub struct GhIssue {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub user: GhUser,
    #[serde(default)]
    pub assignees: Vec<GhUser>,
    #[serde(default)]
    pub labels: Vec<GhLabel>,
    pub created_at: String,
    pub updated_at: String,
    pub html_url: String,
    /// Present if this "issue" is actually a pull request.
    pub pull_request: Option<GhPullRequestRef>,
}

#[derive(Deserialize, Debug)]
pub struct GhUser {
    pub login: String,
}

#[derive(Deserialize, Debug)]
pub struct GhLabel {
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct GhPullRequestRef {
    pub html_url: String,
}

/// GitHub comment from the REST API.
#[derive(Deserialize, Debug)]
pub struct GhComment {
    pub user: GhUser,
    pub body: String,
    pub created_at: String,
}

/// Client for the GitHub REST API.
pub struct GithubClient {
    agent: ureq::Agent,
    token: String,
    pub owner: String,
    pub repo: String,
}

/// Parameters for fetching issues.
pub struct FetchParams {
    pub state: String,
    pub labels: Vec<String>,
    pub since: Option<String>,
    pub limit: Option<usize>,
    pub include_issues: bool,
    pub include_prs: bool,
}

impl GithubClient {
    pub fn new(agent: ureq::Agent, token: String, owner: String, repo: String) -> Self {
        Self {
            agent,
            token,
            owner,
            repo,
        }
    }

    /// Fetch issues (and/or PRs) with pagination.
    pub fn fetch_issues(&self, params: &FetchParams) -> Result<Vec<GhIssue>> {
        let mut all_items = Vec::new();
        let mut page = 1u32;

        loop {
            let mut url = format!(
                "https://api.github.com/repos/{}/{}/issues?state={}&per_page=100&sort=updated&direction=asc&page={}",
                self.owner, self.repo, params.state, page
            );

            if !params.labels.is_empty() {
                url.push_str(&format!("&labels={}", params.labels.join(",")));
            }
            if let Some(ref since) = params.since {
                url.push_str(&format!("&since={since}"));
            }

            let response = self
                .agent
                .get(&url)
                .header("Authorization", &format!("Bearer {}", self.token))
                .header("Accept", "application/vnd.github.v3+json")
                .header("User-Agent", "vex")
                .call()
                .map_err(|e| {
                    if let ureq::Error::StatusCode(status) = &e {
                        match *status {
                            401 => return anyhow::anyhow!(
                                "GitHub authentication failed (HTTP 401). Check your token."
                            ),
                            403 => return anyhow::anyhow!(
                                "GitHub access forbidden (HTTP 403). Your token may lack permissions or you've hit the rate limit."
                            ),
                            404 => return anyhow::anyhow!(
                                "Repository {}/{} not found (HTTP 404). Check the name and your access permissions.",
                                self.owner, self.repo
                            ),
                            _ => {}
                        }
                    }
                    anyhow::anyhow!("GitHub API request failed: {e}")
                })?;

            let body = response
                .into_body()
                .read_to_string()
                .context("Failed to read GitHub API response")?;

            let items: Vec<GhIssue> =
                serde_json::from_str(&body).context("Failed to parse GitHub API response")?;

            if items.is_empty() {
                break;
            }

            // Filter: GitHub's issues endpoint returns both issues and PRs.
            // Items with `pull_request` field are PRs.
            for item in items {
                let is_pr = item.pull_request.is_some();
                if (is_pr && params.include_prs) || (!is_pr && params.include_issues) {
                    all_items.push(item);
                }

                if let Some(limit) = params.limit {
                    if all_items.len() >= limit {
                        return Ok(all_items);
                    }
                }
            }

            page += 1;

            // Safety: don't fetch more than 50 pages (5000 items)
            if page > 50 {
                break;
            }
        }

        Ok(all_items)
    }

    /// Fetch all comments for an issue/PR.
    pub fn fetch_comments(&self, issue_number: u64) -> Result<Vec<GhComment>> {
        let mut all_comments = Vec::new();
        let mut page = 1u32;

        loop {
            let url = format!(
                "https://api.github.com/repos/{}/{}/issues/{}/comments?per_page=100&page={}",
                self.owner, self.repo, issue_number, page
            );

            let response = self
                .agent
                .get(&url)
                .header("Authorization", &format!("Bearer {}", self.token))
                .header("Accept", "application/vnd.github.v3+json")
                .header("User-Agent", "vex")
                .call()
                .with_context(|| {
                    format!("Failed to fetch comments for issue #{issue_number}")
                })?;

            let body = response
                .into_body()
                .read_to_string()
                .context("Failed to read comments response")?;

            let comments: Vec<GhComment> =
                serde_json::from_str(&body).context("Failed to parse comments")?;

            if comments.is_empty() {
                break;
            }

            all_comments.extend(comments);
            page += 1;

            if page > 20 {
                break;
            }
        }

        Ok(all_comments)
    }
}
