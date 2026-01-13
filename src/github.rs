use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// A merged PR with its metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergedPr {
    pub number: u32,
    pub title: String,
    #[serde(rename = "headRefName")]
    pub branch: String,
    pub body: Option<String>,
    #[serde(rename = "mergedAt")]
    pub merged_at: Option<String>,
}

/// PR→Issue→Branch mapping stored in cache
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrMapping {
    pub pr_number: u32,
    pub title: String,
    pub branch: String,
    pub closed_issues: Vec<u32>,
    pub merged_at: Option<String>,
}

/// Cached repo data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoCache {
    pub owner: String,
    pub repo: String,
    pub prs: Vec<PrMapping>,
    pub synced_at: String,
}

/// Get the cache directory path
fn get_cache_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("aist")
        .join("repos")
}

/// Get the cache file path for a repo
fn get_cache_path(owner: &str, repo: &str) -> PathBuf {
    get_cache_dir().join(format!("{}-{}.json", owner, repo))
}

/// Auto-detect repo from git remote
pub fn detect_repo() -> Option<(String, String)> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let url = String::from_utf8_lossy(&output.stdout);
    parse_github_remote(&url)
}

/// Parse owner/repo from git remote URL
fn parse_github_remote(url: &str) -> Option<(String, String)> {
    let url = url.trim();

    // SSH format: git@github.com:owner/repo.git
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        let rest = rest.strip_suffix(".git").unwrap_or(rest);
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() == 2 {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }

    // HTTPS format: https://github.com/owner/repo.git
    if url.contains("github.com") {
        let parts: Vec<&str> = url.split('/').collect();
        if parts.len() >= 2 {
            let owner = parts[parts.len() - 2].to_string();
            let repo = parts[parts.len() - 1]
                .strip_suffix(".git")
                .unwrap_or(parts[parts.len() - 1])
                .to_string();
            return Some((owner, repo));
        }
    }

    None
}

/// Extract closed issue numbers from PR body
fn extract_closed_issues(body: &Option<String>) -> Vec<u32> {
    let body = match body {
        Some(b) => b,
        None => return vec![],
    };

    let mut issues = Vec::new();

    // Match patterns like "Closes #123", "Fixes #456", "Resolves #789"
    // Case insensitive, with optional colon
    let patterns = [
        "closes", "close", "fixes", "fix", "resolves", "resolve", "closed", "fixed", "resolved",
    ];

    let body_lower = body.to_lowercase();

    for pattern in patterns {
        // Find all occurrences of "pattern #N" or "pattern: #N"
        let mut search_start = 0;
        while let Some(pos) = body_lower[search_start..].find(pattern) {
            let abs_pos = search_start + pos + pattern.len();
            search_start = abs_pos;

            // Skip whitespace and optional colon
            let remaining = &body[abs_pos..];
            let remaining = remaining.trim_start();
            let remaining = remaining.strip_prefix(':').unwrap_or(remaining);
            let remaining = remaining.trim_start();

            // Check for #N
            if let Some(rest) = remaining.strip_prefix('#') {
                // Extract the number
                let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(num) = num_str.parse::<u32>() {
                    if !issues.contains(&num) {
                        issues.push(num);
                    }
                }
            }
        }
    }

    issues
}

/// Fetch merged PRs using gh CLI
fn fetch_merged_prs(owner: &str, repo: &str) -> Result<Vec<MergedPr>, String> {
    let output = Command::new("gh")
        .args([
            "pr",
            "list",
            "--repo",
            &format!("{}/{}", owner, repo),
            "--state",
            "merged",
            "--json",
            "number,headRefName,body,mergedAt,title",
            "--limit",
            "100",
        ])
        .output()
        .map_err(|e| format!("Failed to run gh command: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh command failed: {}", stderr));
    }

    let prs: Vec<MergedPr> = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse gh output: {}", e))?;

    Ok(prs)
}

/// Sync GitHub PRs and cache the mappings
pub fn sync(owner: Option<&str>, repo: Option<&str>) -> Result<(), String> {
    // Auto-detect repo if not specified
    let (owner, repo) = match (owner, repo) {
        (Some(o), Some(r)) => (o.to_string(), r.to_string()),
        _ => detect_repo().ok_or_else(|| {
            "Could not detect repo from git remote. Use --owner and --repo flags.".to_string()
        })?,
    };

    println!("{} Syncing {}/{}...", "→".blue(), owner.bold(), repo.bold());

    // Fetch merged PRs
    let prs = fetch_merged_prs(&owner, &repo)?;
    println!("{} Fetched {} merged PRs", "✓".green(), prs.len());

    // Convert to mappings
    let mappings: Vec<PrMapping> = prs
        .into_iter()
        .map(|pr| {
            let closed_issues = extract_closed_issues(&pr.body);
            PrMapping {
                pr_number: pr.number,
                title: pr.title,
                branch: pr.branch,
                closed_issues,
                merged_at: pr.merged_at,
            }
        })
        .collect();

    // Count issues linked
    let issues_count: usize = mappings.iter().map(|m| m.closed_issues.len()).sum();
    println!("{} Found {} linked issues", "✓".green(), issues_count);

    // Create cache directory
    let cache_dir = get_cache_dir();
    fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Failed to create cache directory: {}", e))?;

    // Create cache data
    let cache = RepoCache {
        owner: owner.clone(),
        repo: repo.clone(),
        prs: mappings,
        synced_at: chrono::Utc::now().to_rfc3339(),
    };

    // Write cache file
    let cache_path = get_cache_path(&owner, &repo);
    let json = serde_json::to_string_pretty(&cache)
        .map_err(|e| format!("Failed to serialize cache: {}", e))?;
    fs::write(&cache_path, json).map_err(|e| format!("Failed to write cache file: {}", e))?;

    println!(
        "{} Cached to {}",
        "✓".green(),
        cache_path.display().to_string().dimmed()
    );

    Ok(())
}

/// Load cached repo data
#[allow(dead_code)]
pub fn load_cache(owner: &str, repo: &str) -> Option<RepoCache> {
    let cache_path = get_cache_path(owner, repo);
    let content = fs::read_to_string(&cache_path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Load cache for auto-detected repo
#[allow(dead_code)]
pub fn load_current_repo_cache() -> Option<RepoCache> {
    let (owner, repo) = detect_repo()?;
    load_cache(&owner, &repo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_github_remote_ssh() {
        let url = "git@github.com:owner/repo.git";
        assert_eq!(
            parse_github_remote(url),
            Some(("owner".to_string(), "repo".to_string()))
        );
    }

    #[test]
    fn test_parse_github_remote_https() {
        let url = "https://github.com/owner/repo.git";
        assert_eq!(
            parse_github_remote(url),
            Some(("owner".to_string(), "repo".to_string()))
        );
    }

    #[test]
    fn test_parse_github_remote_https_no_git() {
        let url = "https://github.com/owner/repo";
        assert_eq!(
            parse_github_remote(url),
            Some(("owner".to_string(), "repo".to_string()))
        );
    }

    #[test]
    fn test_extract_closed_issues() {
        let body = Some("Closes #123\nFixes #456".to_string());
        let issues = extract_closed_issues(&body);
        assert_eq!(issues, vec![123, 456]);
    }

    #[test]
    fn test_extract_closed_issues_case_insensitive() {
        let body = Some("CLOSES #1, closes #2, ClOsEs #3".to_string());
        let issues = extract_closed_issues(&body);
        assert_eq!(issues, vec![1, 2, 3]);
    }

    #[test]
    fn test_extract_closed_issues_with_colon() {
        let body = Some("Fixes: #42".to_string());
        let issues = extract_closed_issues(&body);
        assert_eq!(issues, vec![42]);
    }

    #[test]
    fn test_extract_closed_issues_none() {
        let body: Option<String> = None;
        let issues = extract_closed_issues(&body);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_extract_closed_issues_no_matches() {
        let body = Some("Just a regular PR description".to_string());
        let issues = extract_closed_issues(&body);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_extract_closed_issues_dedup() {
        let body = Some("Closes #5\nFixes #5".to_string());
        let issues = extract_closed_issues(&body);
        assert_eq!(issues, vec![5]);
    }
}
