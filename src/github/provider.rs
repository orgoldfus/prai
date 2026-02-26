use std::future::Future;

use anyhow::Result;

use super::types::{PullRequest, ReviewThread};

/// Abstraction over a git hosting provider (GitHub, GitLab, etc.).
///
/// Implementations shell out to the provider's CLI or hit its API.
/// This trait is object-safe and async-ready via [`tokio`].
#[allow(dead_code)]
pub trait GitProvider: Send + Sync {
    /// List open pull requests authored by the given user.
    fn list_open_prs(
        &self,
        author: &str,
    ) -> impl Future<Output = Result<Vec<PullRequest>>> + Send;

    /// Look up a single PR by number.
    fn get_pr(&self, number: u64) -> impl Future<Output = Result<PullRequest>> + Send;

    /// Fetch the PR title/body for context.
    fn get_pr_details(&self, number: u64) -> impl Future<Output = Result<PullRequest>> + Send;

    /// Fetch all review threads (with comments) for a PR.
    fn get_review_threads(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> impl Future<Output = Result<Vec<ReviewThread>>> + Send;

    /// Find an open PR whose head branch matches `branch`.
    fn find_pr_for_branch(
        &self,
        branch: &str,
    ) -> impl Future<Output = Result<Option<PullRequest>>> + Send;

    /// Add a reaction emoji to a comment.
    fn add_reaction(
        &self,
        owner: &str,
        repo: &str,
        comment_id: &str,
        reaction: &str,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Post a reply to a review thread.
    fn reply_to_comment(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        comment_id: &str,
        body: &str,
    ) -> impl Future<Output = Result<()>> + Send;
}
