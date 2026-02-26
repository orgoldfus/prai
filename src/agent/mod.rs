pub mod cursor;
pub mod provider;

use crate::github::types::{PullRequest, ReviewComment};

/// Build the prompt that will be sent to the AI agent.
///
/// When multiple comments are provided they are all included so the agent
/// can address them in a single pass.
pub fn build_prompt(pr: &PullRequest, comments: &[&ReviewComment]) -> String {
    let mut prompt = String::from(
        "You are fixing code review comments on a pull request.\n\n",
    );

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
