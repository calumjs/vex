//! File discovery helpers: type-name extraction, import following,
//! git co-change analysis, and auto synonym expansion.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Extract PascalCase/camelCase type names (>= 6 chars) from file content.
/// These are likely class/struct/interface names that might be file names elsewhere.
pub fn extract_type_names(content: &str) -> Vec<String> {
    let mut names = HashSet::new();
    for word in content.split(|c: char| !c.is_alphanumeric() && c != '_') {
        if word.len() >= 6
            && word.chars().next().is_some_and(|c| c.is_uppercase())
            && word.chars().any(|c| c.is_lowercase())
        {
            names.insert(word.to_string());
        }
    }
    names.into_iter().collect()
}

/// Parse import/using statements from source code and return referenced names.
/// Handles C#, Rust, Python, TypeScript, JavaScript, Go, Java.
pub fn extract_imports(content: &str) -> Vec<String> {
    let mut imports = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();

        // C#: using Namespace.ClassName;
        if let Some(rest) = trimmed.strip_prefix("using ") {
            if let Some(path) = rest.strip_suffix(';') {
                let path = path.trim();
                if !path.starts_with('(') && !path.contains('=') {
                    // Take the last segment as the type name
                    if let Some(name) = path.rsplit('.').next() {
                        if name.len() >= 4 {
                            imports.push(name.to_string());
                        }
                    }
                }
            }
        }
        // Rust: use crate::module::Type;
        else if let Some(rest) = trimmed.strip_prefix("use ") {
            if let Some(path) = rest.strip_suffix(';') {
                if let Some(name) = path.rsplit("::").next() {
                    let name = name.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
                    if name.len() >= 4 {
                        imports.push(name.to_string());
                    }
                }
            }
        }
        // Python: from module import thing / import module
        else if trimmed.starts_with("from ") || trimmed.starts_with("import ") {
            for word in trimmed.split_whitespace() {
                if word.len() >= 4
                    && word != "from"
                    && word != "import"
                    && word != "as"
                    && word.chars().next().is_some_and(|c| c.is_alphabetic())
                {
                    let name = word.split('.').last().unwrap_or(word);
                    if name.len() >= 4 {
                        imports.push(name.to_string());
                    }
                }
            }
        }
        // TypeScript/JavaScript: import { Thing } from './path'
        else if trimmed.starts_with("import ") {
            for word in trimmed
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .filter(|w| w.len() >= 4)
            {
                if word != "import" && word != "from" && word != "require" {
                    imports.push(word.to_string());
                }
            }
        }
    }
    imports
}

/// Find files that frequently co-change with the given file paths in git history.
/// Returns file paths sorted by co-change frequency.
pub fn git_cochange_files(matched_files: &[&Path], limit: usize) -> Vec<PathBuf> {
    // Get recent commits touching matched files
    let output = std::process::Command::new("git")
        .args(["log", "--format=%H", "--name-only", "-n", "200"])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let log = String::from_utf8_lossy(&output.stdout);

    // Parse commits: group files by commit hash
    let matched_set: HashSet<String> = matched_files
        .iter()
        .filter_map(|p| p.to_str())
        .map(|s| s.replace('\\', "/"))
        .collect();

    let mut cochange_counts: HashMap<String, usize> = HashMap::new();
    let mut current_files: Vec<String> = Vec::new();
    let mut has_matched = false;

    for line in log.lines() {
        if line.is_empty() {
            // End of commit — if this commit touched a matched file,
            // increment co-change count for all other files in it
            if has_matched {
                for f in &current_files {
                    if !matched_set.contains(f) {
                        *cochange_counts.entry(f.clone()).or_insert(0) += 1;
                    }
                }
            }
            current_files.clear();
            has_matched = false;
        } else if line.len() == 40 && line.chars().all(|c| c.is_ascii_hexdigit()) {
            // Commit hash — skip
        } else {
            let normalized = line.replace('\\', "/");
            if matched_set.contains(&normalized) {
                has_matched = true;
            }
            current_files.push(normalized);
        }
    }

    // Sort by frequency
    let mut pairs: Vec<_> = cochange_counts.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1));
    pairs
        .into_iter()
        .take(limit)
        .filter(|(_, count)| *count >= 2)
        .map(|(path, _)| PathBuf::from(path))
        .collect()
}

/// Auto-expand query with synonyms from a static mapping of common programming concepts.
pub fn auto_synonyms(query: &str) -> Vec<String> {
    let q = query.to_lowercase();
    let mut synonyms = Vec::new();

    let mappings: &[(&[&str], &[&str])] = &[
        (
            &["race", "concurrent", "thread", "parallel", "deadlock"],
            &["lock", "mutex", "semaphore", "atomic", "synchronize"],
        ),
        (
            &["notify", "notification", "alert"],
            &["email", "smtp", "push", "signal", "hub", "message"],
        ),
        (
            &["background", "scheduled", "cron", "recurring"],
            &["job", "worker", "queue", "hangfire", "hosted"],
        ),
        (
            &["auth", "login", "signin", "credential"],
            &["token", "jwt", "session", "identity", "oauth"],
        ),
        (
            &["cache", "caching", "memoize"],
            &["redis", "memory", "invalidate", "expire", "ttl"],
        ),
        (
            &["retry", "resilience", "fault"],
            &["polly", "backoff", "circuit", "fallback", "timeout"],
        ),
        (
            &["database", "query", "persistence"],
            &["repository", "entity", "dbcontext", "migration", "sql"],
        ),
        (
            &["endpoint", "route", "api"],
            &["controller", "handler", "middleware", "request"],
        ),
        (
            &["upload", "file", "storage"],
            &["blob", "stream", "multipart", "bucket"],
        ),
        (
            &["logging", "observability", "tracing"],
            &["logger", "serilog", "telemetry", "diagnostic", "monitor"],
        ),
        (
            &["permission", "authorization", "access"],
            &["role", "policy", "claim", "authorize", "rbac"],
        ),
        (
            &["validate", "validation"],
            &["validator", "fluent", "rule", "constraint"],
        ),
        (
            &["test", "testing"],
            &["mock", "fixture", "assert", "xunit", "fake"],
        ),
        (
            &["deploy", "deployment", "ci", "pipeline"],
            &["docker", "kubernetes", "helm", "release", "workflow"],
        ),
        (
            &["encrypt", "encryption", "secret", "security"],
            &["cipher", "hash", "vault", "certificate", "tls"],
        ),
        (
            &["webhook", "callback", "event"],
            &["subscribe", "publish", "dispatch", "listener", "handler"],
        ),
    ];

    for (triggers, terms) in mappings {
        if triggers.iter().any(|t| q.contains(t)) {
            for &term in *terms {
                if !q.contains(term) && !synonyms.contains(&term.to_string()) {
                    synonyms.push(term.to_string());
                }
            }
        }
    }

    synonyms
}
