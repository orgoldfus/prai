use serde::Deserialize;

/// A GitHub pull request.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    #[serde(default)]
    pub body: String,
    pub url: String,
    pub head_ref_name: String,
    #[serde(default)]
    pub base_ref_name: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub author: PrAuthor,
}

/// The author of a pull request (from `gh` JSON).
#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)]
pub struct PrAuthor {
    #[serde(default)]
    pub login: String,
}

/// A review thread on a pull request, containing one or more comments.
#[derive(Debug, Clone)]
pub struct ReviewThread {
    pub is_resolved: bool,
    pub comments: Vec<ReviewComment>,
}

impl ReviewThread {
    /// Returns the root (first) comment of the thread, if any.
    pub fn root_comment(&self) -> Option<&ReviewComment> {
        self.comments.first()
    }
}

/// A single review comment within a thread.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ReviewComment {
    pub id: String,
    pub body: String,
    pub path: String,
    pub line: Option<u32>,
    pub start_line: Option<u32>,
    pub diff_hunk: String,
    pub author: String,
    pub created_at: String,
    pub url: String,
}
