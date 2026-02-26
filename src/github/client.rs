use anyhow::{Context, Result, bail};
use serde_json::Value;
use tokio::process::Command;

use super::provider::GitProvider;
use super::types::{PullRequest, ReviewComment, ReviewThread};

/// GitHub implementation of [`GitProvider`], backed by the `gh` CLI.
#[derive(Debug, Default)]
pub struct GitHubClient;

impl GitHubClient {
    /// Run `gh` with the given arguments and return stdout as a string.
    async fn gh(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("gh")
            .args(args)
            .output()
            .await
            .context("failed to execute `gh` CLI")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("gh command failed: {stderr}");
        }

        Ok(String::from_utf8(output.stdout)?)
    }

    /// Run a GraphQL query via `gh api graphql`.
    async fn graphql(&self, query: &str) -> Result<Value> {
        let output = self.gh(&["api", "graphql", "-f", &format!("query={query}")]).await?;
        let value: Value = serde_json::from_str(&output)
            .context("failed to parse GraphQL response")?;

        if let Some(errors) = value.get("errors") {
            bail!("GraphQL errors: {errors}");
        }

        Ok(value)
    }

    /// Verify that `gh` is installed and authenticated.
    pub async fn check_auth() -> Result<()> {
        let output = Command::new("gh")
            .args(["auth", "status"])
            .output()
            .await
            .context(
                "GitHub CLI (gh) is not installed.\n\
                 Install it: https://cli.github.com",
            )?;

        if !output.status.success() {
            bail!(
                "GitHub CLI is not authenticated.\n\
                 Run `gh auth login` to sign in."
            );
        }

        Ok(())
    }
}

impl GitProvider for GitHubClient {
    async fn list_open_prs(&self, author: &str) -> Result<Vec<PullRequest>> {
        let json = self
            .gh(&[
                "pr",
                "list",
                "--author",
                author,
                "--state",
                "open",
                "--json",
                "number,title,body,url,headRefName,baseRefName,createdAt,author",
                "--limit",
                "50",
            ])
            .await?;

        let prs: Vec<PullRequest> =
            serde_json::from_str(&json).context("failed to parse PR list")?;
        Ok(prs)
    }

    async fn get_pr(&self, number: u64) -> Result<PullRequest> {
        let json = self
            .gh(&[
                "pr",
                "view",
                &number.to_string(),
                "--json",
                "number,title,body,url,headRefName,baseRefName,createdAt,author",
            ])
            .await?;

        let pr: PullRequest = serde_json::from_str(&json).context("failed to parse PR")?;
        Ok(pr)
    }

    async fn get_pr_details(&self, number: u64) -> Result<PullRequest> {
        self.get_pr(number).await
    }

    async fn get_review_threads(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<Vec<ReviewThread>> {
        // GraphQL is required to get `isResolved` on review threads.
        let query = format!(
            r#"{{
              repository(owner: "{owner}", name: "{repo}") {{
                pullRequest(number: {pr_number}) {{
                  reviewThreads(first: 100) {{
                    nodes {{
                      isResolved
                      comments(first: 50) {{
                        nodes {{
                          id
                          body
                          path
                          line
                          startLine
                          diffHunk
                          author {{ login }}
                          createdAt
                          url
                        }}
                      }}
                    }}
                  }}
                }}
              }}
            }}"#
        );

        let value = self.graphql(&query).await?;

        let threads_json = &value["data"]["repository"]["pullRequest"]["reviewThreads"]["nodes"];
        let Some(threads_arr) = threads_json.as_array() else {
            bail!("unexpected GraphQL response shape");
        };

        let mut threads = Vec::new();
        for thread in threads_arr {
            let is_resolved = thread["isResolved"].as_bool().unwrap_or(false);
            let comments_json = &thread["comments"]["nodes"];
            let Some(comments_arr) = comments_json.as_array() else {
                continue;
            };

            let comments: Vec<ReviewComment> = comments_arr
                .iter()
                .map(|c| ReviewComment {
                    id: c["id"].as_str().unwrap_or_default().to_owned(),
                    body: c["body"].as_str().unwrap_or_default().to_owned(),
                    path: c["path"].as_str().unwrap_or_default().to_owned(),
                    line: c["line"].as_u64().map(|n| n as u32),
                    start_line: c["startLine"].as_u64().map(|n| n as u32),
                    diff_hunk: c["diffHunk"].as_str().unwrap_or_default().to_owned(),
                    author: c["author"]["login"]
                        .as_str()
                        .unwrap_or("unknown")
                        .to_owned(),
                    created_at: c["createdAt"].as_str().unwrap_or_default().to_owned(),
                    url: c["url"].as_str().unwrap_or_default().to_owned(),
                })
                .collect();

            threads.push(ReviewThread {
                is_resolved,
                comments,
            });
        }

        Ok(threads)
    }

    async fn find_pr_for_branch(&self, branch: &str) -> Result<Option<PullRequest>> {
        let json = self
            .gh(&[
                "pr",
                "list",
                "--head",
                branch,
                "--state",
                "open",
                "--json",
                "number,title,body,url,headRefName,baseRefName,createdAt,author",
                "--limit",
                "1",
            ])
            .await?;

        let prs: Vec<PullRequest> =
            serde_json::from_str(&json).context("failed to parse PR list")?;
        Ok(prs.into_iter().next())
    }

    async fn add_reaction(
        &self,
        owner: &str,
        repo: &str,
        comment_id: &str,
        reaction: &str,
    ) -> Result<()> {
        // GraphQL mutation to add a reaction to a review comment.
        let query = format!(
            r#"mutation {{
              addReaction(input: {{subjectId: "{comment_id}", content: {reaction}}}) {{
                reaction {{ content }}
              }}
            }}"#
        );

        self.graphql(&query).await?;
        let _ = (owner, repo); // reserved for future REST fallback
        Ok(())
    }

    async fn reply_to_comment(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        comment_id: &str,
        body: &str,
    ) -> Result<()> {
        // Use REST API to reply to a pull request review comment.
        let endpoint = format!(
            "repos/{owner}/{repo}/pulls/{pr_number}/comments/{comment_id}/replies"
        );
        let body_escaped = body.replace('"', r#"\""#);

        self.gh(&[
            "api",
            &endpoint,
            "-f",
            &format!("body={body_escaped}"),
            "--method",
            "POST",
        ])
        .await?;

        Ok(())
    }
}
