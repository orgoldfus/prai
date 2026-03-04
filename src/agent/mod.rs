pub mod cursor;
pub mod provider;
pub mod stream;

use crate::github::types::{PullRequest, ReviewComment};

/// Build the prompt that will be sent to the AI agent.
///
/// When multiple comments are provided they are all included so the agent
/// can address them in a single pass.
#[allow(dead_code)]
pub fn build_prompt(pr: &PullRequest, comments: &[&ReviewComment]) -> String {
    build_prompt_with_additional(pr, comments, None)
}

pub fn build_prompt_with_additional(
    pr: &PullRequest,
    comments: &[&ReviewComment],
    additional_instructions: Option<&str>,
) -> String {
    let mut prompt = String::from("You are fixing code review comments on a pull request.\n\n");

    // ── PR context ────────────────────────────────────────────────────
    prompt.push_str("## PR Context\n");
    prompt.push_str(&format!("- **Title:** {}\n", pr.title));
    if !pr.body.is_empty() {
        // Truncate very long descriptions to keep the prompt focused.
        let desc = if pr.body.len() > 2000 {
            format!("{}…", &pr.body[..2000])
        } else {
            pr.body.clone()
        };
        prompt.push_str(&format!("- **Description:**\n{desc}\n"));
    }
    prompt.push('\n');

    // ── Review comments ───────────────────────────────────────────────
    if comments.len() == 1 {
        let c = comments[0];
        prompt.push_str("## Review Comment\n");
        write_comment(&mut prompt, c);
    } else {
        prompt.push_str(&format!(
            "## Review Comments ({} comments)\n",
            comments.len()
        ));
        for (i, c) in comments.iter().enumerate() {
            prompt.push_str(&format!("### Comment {}\n", i + 1));
            write_comment(&mut prompt, c);
        }
    }

    // ── Instructions ──────────────────────────────────────────────────
    prompt.push_str(
        "\n## Instructions\n\
         Please fix the code to address the review comment(s) above.\n\
         - Make the minimal necessary changes.\n\
         - Do not change any unrelated code.\n\
         - If a comment is unclear, make your best judgement.\n",
    );

    if let Some(extra) = additional_instructions
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        prompt.push_str("\n## Additional Instructions\n");
        prompt.push_str(extra);
        prompt.push('\n');
    }

    prompt
}

fn write_comment(buf: &mut String, c: &ReviewComment) {
    buf.push_str(&format!("- **File:** {}\n", c.path));
    if let Some(line) = c.line {
        buf.push_str(&format!("- **Line:** {line}\n"));
    }
    buf.push_str(&format!("- **Author:** @{}\n", c.author));
    buf.push_str(&format!("- **Comment:** {}\n", c.body));
    if !c.diff_hunk.is_empty() {
        buf.push_str("- **Code Context (diff hunk):**\n```diff\n");
        buf.push_str(&c.diff_hunk);
        buf.push_str("\n```\n");
    }
    buf.push('\n');
}

#[cfg(test)]
mod tests {
    use super::build_prompt_with_additional;
    use crate::github::types::{PrAuthor, PullRequest, ReviewComment};

    fn sample_pr() -> PullRequest {
        PullRequest {
            number: 42,
            title: "Improve parser".to_owned(),
            body: "Refactor parser for readability.".to_owned(),
            url: "https://example.com/pr/42".to_owned(),
            head_ref_name: "feature/parser".to_owned(),
            base_ref_name: "main".to_owned(),
            created_at: String::new(),
            author: PrAuthor::default(),
        }
    }

    fn sample_comment() -> ReviewComment {
        ReviewComment {
            id: "c1".to_owned(),
            body: "Handle empty input.".to_owned(),
            path: "src/parser.rs".to_owned(),
            line: Some(12),
            start_line: None,
            diff_hunk: "@@ -1,3 +1,3 @@".to_owned(),
            author: "reviewer".to_owned(),
            created_at: String::new(),
            url: "https://example.com/comment/1".to_owned(),
            has_thumbs_up: false,
        }
    }

    #[test]
    fn none_additional_instructions_does_not_add_section() {
        let pr = sample_pr();
        let comment = sample_comment();
        let prompt = build_prompt_with_additional(&pr, &[&comment], None);

        assert!(!prompt.contains("## Additional Instructions"));
    }

    #[test]
    fn blank_additional_instructions_are_ignored() {
        let pr = sample_pr();
        let comment = sample_comment();
        let prompt = build_prompt_with_additional(&pr, &[&comment], Some("  \n\t  "));

        assert!(!prompt.contains("## Additional Instructions"));
    }

    #[test]
    fn additional_instructions_are_appended_once() {
        let pr = sample_pr();
        let comment = sample_comment();
        let prompt = build_prompt_with_additional(
            &pr,
            &[&comment],
            Some("Follow project style.\nAvoid renaming public APIs."),
        );

        assert_eq!(prompt.matches("## Additional Instructions").count(), 1);
        let base_idx = prompt.find("## Instructions").unwrap();
        let add_idx = prompt.find("## Additional Instructions").unwrap();
        assert!(add_idx > base_idx);
    }

    #[test]
    fn multi_comment_prompt_numbers_comments() {
        let pr = sample_pr();
        let c1 = sample_comment();
        let mut c2 = sample_comment();
        c2.id = "c2".to_owned();
        c2.body = "Fix naming.".to_owned();
        c2.path = "src/util.rs".to_owned();

        let prompt = build_prompt_with_additional(&pr, &[&c1, &c2], None);

        assert!(prompt.contains("## Review Comments (2 comments)"));
        assert!(prompt.contains("### Comment 1"));
        assert!(prompt.contains("### Comment 2"));
        assert!(prompt.contains("Handle empty input."));
        assert!(prompt.contains("Fix naming."));
    }

    #[test]
    fn long_pr_body_is_truncated() {
        let mut pr = sample_pr();
        pr.body = "x".repeat(3000);
        let comment = sample_comment();

        let prompt = build_prompt_with_additional(&pr, &[&comment], None);

        assert!(prompt.contains('…'));
        assert!(!prompt.contains(&"x".repeat(3000)));
    }

    #[test]
    fn single_comment_uses_singular_heading() {
        let pr = sample_pr();
        let comment = sample_comment();
        let prompt = build_prompt_with_additional(&pr, &[&comment], None);

        assert!(prompt.contains("## Review Comment\n"));
        assert!(!prompt.contains("### Comment 1"));
    }
}
