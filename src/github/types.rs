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
    pub id: String,
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
pub struct ReviewComment {
    pub id: String,
    pub body: String,
    pub path: String,
    pub line: Option<u32>,
    #[allow(dead_code)]
    pub start_line: Option<u32>,
    pub diff_hunk: String,
    pub author: String,
    pub created_at: String,
    pub url: String,
    pub has_thumbs_up: bool,
}

impl ReviewComment {
    /// Build a `ReviewComment` from a UI `CommentEntry`.
    ///
    /// Fields that don't exist on the entry (`start_line`, `created_at`,
    /// `has_thumbs_up`) are filled with defaults.
    pub fn from_entry(
        comment_id: &str,
        body: &str,
        path: &str,
        line: Option<u32>,
        diff_hunk: &str,
        author: &str,
        url: &str,
    ) -> Self {
        Self {
            id: comment_id.to_owned(),
            body: body.to_owned(),
            path: path.to_owned(),
            line,
            start_line: None,
            diff_hunk: diff_hunk.to_owned(),
            author: author.to_owned(),
            created_at: String::new(),
            url: url.to_owned(),
            has_thumbs_up: false,
        }
    }
}
