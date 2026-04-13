use super::api::{GhComment, GhIssue};

/// Render a GitHub issue/PR as a Markdown document with YAML frontmatter.
pub fn render_issue(issue: &GhIssue, comments: &[GhComment]) -> String {
    let kind = if issue.pull_request.is_some() {
        "pull_request"
    } else {
        "issue"
    };
    let kind_label = if issue.pull_request.is_some() {
        "PR"
    } else {
        "Issue"
    };

    let labels = issue
        .labels
        .iter()
        .map(|l| l.name.as_str())
        .collect::<Vec<_>>();
    let assignees = issue
        .assignees
        .iter()
        .map(|a| a.login.as_str())
        .collect::<Vec<_>>();

    let mut out = String::with_capacity(4096);

    // YAML frontmatter
    out.push_str("---\n");
    out.push_str(&format!("type: {kind}\n"));
    out.push_str(&format!("number: {}\n", issue.number));
    out.push_str(&format!("title: \"{}\"\n", escape_yaml(&issue.title)));
    out.push_str(&format!("state: {}\n", issue.state));
    out.push_str(&format!("author: {}\n", issue.user.login));
    if !assignees.is_empty() {
        out.push_str(&format!("assignees: [{}]\n", assignees.join(", ")));
    }
    if !labels.is_empty() {
        out.push_str(&format!("labels: [{}]\n", labels.join(", ")));
    }
    out.push_str(&format!("created: {}\n", issue.created_at));
    out.push_str(&format!("updated: {}\n", issue.updated_at));
    out.push_str(&format!("url: {}\n", issue.html_url));
    out.push_str("---\n\n");

    // Title
    out.push_str(&format!(
        "# {kind_label} #{}: {}\n\n",
        issue.number, issue.title
    ));

    // Body
    if let Some(ref body) = issue.body {
        let body = body.trim();
        if !body.is_empty() {
            out.push_str(body);
            out.push_str("\n\n");
        }
    }

    // Comments
    if !comments.is_empty() {
        out.push_str("---\n\n");
        out.push_str("## Comments\n\n");
        for comment in comments {
            out.push_str(&format!(
                "### @{} ({})\n\n",
                comment.user.login, comment.created_at
            ));
            out.push_str(comment.body.trim());
            out.push_str("\n\n");
        }
    }

    out
}

/// Generate a stable slug from an issue title.
///
/// "Fix login timeout!" → "fix-login-timeout"
pub fn slugify(title: &str) -> String {
    let slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();

    slug.split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(50)
        .collect()
}

/// Minimal YAML string escaping — handle quotes in titles.
fn escape_yaml(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
