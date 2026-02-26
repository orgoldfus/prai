use std::process::Command;

use anyhow::{Context, Result, bail};

/// Information about the current git repository.
#[derive(Debug, Clone)]
pub struct RepoInfo {
    pub owner: String,
    pub name: String,
}

/// Check whether the current directory is inside a git repository.
pub fn is_git_repo() -> bool {
    Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Return the current branch name.
pub fn current_branch() -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .context("failed to run git")?;

    if !output.status.success() {
        bail!("failed to determine current branch");
    }

    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

/// Extract `owner/repo` from the git remote URL.
///
/// Supports both SSH (`git@github.com:owner/repo.git`) and HTTPS
/// (`https://github.com/owner/repo.git`) formats.
pub fn repo_info() -> Result<RepoInfo> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .context("failed to get git remote URL")?;

    if !output.status.success() {
        bail!("no 'origin' remote found");
    }

    let url = String::from_utf8(output.stdout)?.trim().to_owned();
    parse_remote_url(&url)
}

/// Parse an owner/repo pair out of a remote URL string.
fn parse_remote_url(url: &str) -> Result<RepoInfo> {
    // SSH: git@github.com:owner/repo.git
    if let Some(path) = url.strip_prefix("git@") {
        let path = path
            .split_once(':')
            .map(|(_, p)| p)
            .unwrap_or(path);
        return parse_owner_repo(path);
    }

    // HTTPS: https://github.com/owner/repo.git
    if let Some(path) = url.strip_prefix("https://") {
        // Skip the host part (github.com/)
        let path = path
            .split_once('/')
            .map(|(_, p)| p)
            .unwrap_or(path);
        return parse_owner_repo(path);
    }

    bail!("unrecognised remote URL format: {url}");
}

fn parse_owner_repo(path: &str) -> Result<RepoInfo> {
    let path = path.trim_end_matches(".git");
    let (owner, name) = path
        .split_once('/')
        .context("remote URL does not contain owner/repo")?;

    Ok(RepoInfo {
        owner: owner.to_owned(),
        name: name.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ssh_url() {
        let info = parse_remote_url("git@github.com:orgoldfus/prai.git").unwrap();
        assert_eq!(info.owner, "orgoldfus");
        assert_eq!(info.name, "prai");
    }

    #[test]
    fn parse_https_url() {
        let info =
            parse_remote_url("https://github.com/orgoldfus/prai.git").unwrap();
        assert_eq!(info.owner, "orgoldfus");
        assert_eq!(info.name, "prai");
    }

    #[test]
    fn parse_https_url_without_git_suffix() {
        let info =
            parse_remote_url("https://github.com/orgoldfus/prai").unwrap();
        assert_eq!(info.owner, "orgoldfus");
        assert_eq!(info.name, "prai");
    }
}
