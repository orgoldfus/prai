use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::agent::stream::AgentStreamEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentOutputMode {
    Ui,
    Raw,
}

impl AgentOutputMode {
    pub fn toggle(self) -> Self {
        match self {
            AgentOutputMode::Ui => AgentOutputMode::Raw,
            AgentOutputMode::Raw => AgentOutputMode::Ui,
        }
    }
}

#[derive(Debug, Clone)]
enum TimelineNode {
    Thinking {
        text: String,
        started_at: Instant,
        finished_at: Instant,
    },
    Assistant {
        text: String,
    },
    Tool {
        name: String,
        detail: Option<String>,
        status: ToolStatus,
        started_at: Instant,
        finished_at: Option<Instant>,
    },
    System {
        text: String,
    },
    Error {
        text: String,
    },
    Done {
        text: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolStatus {
    Running,
    Success,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActivityKind {
    Explore,
    Edit,
    Run,
    Other,
}

impl ActivityKind {
    fn done_verb(self) -> &'static str {
        match self {
            ActivityKind::Explore => "explored",
            ActivityKind::Edit => "edited",
            ActivityKind::Run => "ran",
            ActivityKind::Other => "used",
        }
    }

    fn running_verb(self) -> &'static str {
        match self {
            ActivityKind::Explore => "exploring",
            ActivityKind::Edit => "editing",
            ActivityKind::Run => "running",
            ActivityKind::Other => "using",
        }
    }
}

#[derive(Debug, Clone)]
struct ActiveThinking {
    text: String,
    started_at: Instant,
}

#[derive(Debug, Clone)]
struct ToolDigest {
    kind: ActivityKind,
    subject: Option<String>,
    change_summary: Option<String>,
    detail_hint: Option<String>,
}

pub struct AgentTimeline {
    raw_logs: VecDeque<String>,
    nodes: Vec<TimelineNode>,
    active_thinking: Option<ActiveThinking>,
    active_tool_idx: Option<usize>,
    max_raw_lines: usize,
    max_nodes: usize,
    max_text_chars: usize,
}

impl AgentTimeline {
    pub fn new(max_raw_lines: usize, max_nodes: usize, max_text_chars: usize) -> Self {
        Self {
            raw_logs: VecDeque::new(),
            nodes: Vec::new(),
            active_thinking: None,
            active_tool_idx: None,
            max_raw_lines,
            max_nodes,
            max_text_chars,
        }
    }

    pub fn push_raw_line(&mut self, line: String) {
        self.raw_logs.push_back(line);
        while self.raw_logs.len() > self.max_raw_lines {
            self.raw_logs.pop_front();
        }
    }

    pub fn raw_logs(&self) -> &VecDeque<String> {
        &self.raw_logs
    }

    pub fn apply_event(&mut self, event: AgentStreamEvent) {
        match event {
            AgentStreamEvent::ThinkingDelta(text) => self.append_thinking(text),
            AgentStreamEvent::ThinkingDone => self.finish_thinking(),
            AgentStreamEvent::AssistantDelta(text) => {
                self.finish_thinking();
                self.append_assistant(text);
            }
            AgentStreamEvent::ToolStart { name, detail } => {
                self.finish_thinking();
                self.start_tool(name, detail);
            }
            AgentStreamEvent::ToolUpdate { name, detail } => {
                self.finish_thinking();
                self.update_tool(name, detail);
            }
            AgentStreamEvent::ToolEnd {
                name,
                detail,
                success,
            } => {
                self.finish_thinking();
                self.finish_tool(name, detail, success);
            }
            AgentStreamEvent::Error(text) => {
                self.finish_thinking();
                self.push_node(TimelineNode::Error { text });
            }
            AgentStreamEvent::Info(text) => {
                self.finish_thinking();
                self.push_node(TimelineNode::System { text });
            }
            AgentStreamEvent::Done => {
                self.finish_thinking();
                self.push_done_if_missing("Completed".to_owned());
            }
        }
    }

    pub fn mark_complete(&mut self, success: bool, message: Option<&str>) {
        self.finish_thinking();

        if let Some(idx) = self.active_tool_idx.take() {
            if let Some(TimelineNode::Tool {
                status,
                finished_at,
                ..
            }) = self.nodes.get_mut(idx)
            {
                *status = if success {
                    ToolStatus::Success
                } else {
                    ToolStatus::Failed
                };
                *finished_at = Some(Instant::now());
            }
        }

        if success {
            self.push_done_if_missing("Completed".to_owned());
            return;
        }

        let text = message
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .unwrap_or("Agent failed")
            .to_owned();
        self.push_node(TimelineNode::Error { text });
    }

    pub fn ui_lines(&self, pulse_on: bool) -> Vec<String> {
        let mut lines = Vec::new();

        if let Some(summary) = self.activity_summary_line() {
            lines.push(format!("• {summary}"));
        }

        for node in &self.nodes {
            match node {
                TimelineNode::Thinking {
                    text,
                    started_at,
                    finished_at,
                } => {
                    let elapsed = finished_at.duration_since(*started_at);
                    if text.is_empty() {
                        lines.push(format!("💭 thought for {}", format_duration(elapsed)));
                    } else {
                        lines.push(format!(
                            "💭 thought for {}: {}",
                            format_duration(elapsed),
                            compact_inline(text, 88)
                        ));
                    }
                }
                TimelineNode::Assistant { text } => {
                    lines.push("🤖 Assistant".to_owned());
                    for line in text.lines() {
                        lines.push(format!("  {line}"));
                    }
                }
                TimelineNode::Tool {
                    name,
                    detail,
                    status,
                    started_at,
                    finished_at,
                } => {
                    let icon = match status {
                        ToolStatus::Running => "⏳",
                        ToolStatus::Success => "✅",
                        ToolStatus::Failed => "❌",
                    };
                    let elapsed = finished_at
                        .unwrap_or_else(Instant::now)
                        .duration_since(*started_at);
                    let digest = summarize_tool(name, detail.as_deref());
                    let verb = if *status == ToolStatus::Running {
                        digest.kind.running_verb()
                    } else {
                        digest.kind.done_verb()
                    };

                    let mut row = format!("{icon} {verb} via `{}`", compact_inline(name, 32));
                    if let Some(subject) = digest.subject {
                        row.push(' ');
                        row.push_str(&subject);
                    }
                    if let Some(changes) = digest.change_summary {
                        row.push(' ');
                        row.push_str(&changes);
                    }
                    row.push_str(&format!(" ({})", format_duration(elapsed)));
                    lines.push(row);

                    if let Some(detail_hint) = digest.detail_hint {
                        lines.push(format!("  ↳ {detail_hint}"));
                    }
                }
                TimelineNode::System { text } => {
                    lines.push(format!("• {text}"));
                }
                TimelineNode::Error { text } => {
                    lines.push(format!("❌ {text}"));
                }
                TimelineNode::Done { text } => {
                    lines.push(format!("✅ {text}"));
                }
            }
        }

        if let Some(thinking) = &self.active_thinking {
            let icon = if pulse_on { "◉" } else { "○" };
            let elapsed = thinking.started_at.elapsed();
            if thinking.text.is_empty() {
                lines.push(format!(
                    "{icon} Thinking for {}...",
                    format_duration(elapsed)
                ));
            } else {
                lines.push(format!(
                    "{icon} Thinking for {}: {}",
                    format_duration(elapsed),
                    compact_inline(&thinking.text, 96)
                ));
            }
        }

        if lines.is_empty() {
            lines.push("Waiting for agent output...".to_owned());
        }

        lines
    }

    fn append_thinking(&mut self, delta: String) {
        let text = delta.trim();
        if text.is_empty() {
            return;
        }

        if self.active_thinking.is_none() {
            self.active_thinking = Some(ActiveThinking {
                text: String::new(),
                started_at: Instant::now(),
            });
        }

        if let Some(thinking) = &mut self.active_thinking {
            if needs_separator(&thinking.text, text) {
                thinking.text.push(' ');
            }
            thinking.text.push_str(text);
            let char_count = thinking.text.chars().count();
            if char_count > 120 {
                let tail = thinking
                    .text
                    .chars()
                    .skip(char_count - 120)
                    .collect::<String>();
                thinking.text = format!("…{tail}");
            }
        }
    }

    fn finish_thinking(&mut self) {
        let Some(thinking) = self.active_thinking.take() else {
            return;
        };

        self.push_node(TimelineNode::Thinking {
            text: thinking.text,
            started_at: thinking.started_at,
            finished_at: Instant::now(),
        });
    }

    fn append_assistant(&mut self, delta: String) {
        let text = delta.trim();
        if text.is_empty() {
            return;
        }

        if let Some(TimelineNode::Assistant { text: existing }) = self.nodes.last_mut() {
            if !existing.is_empty() {
                existing.push('\n');
            }
            existing.push_str(text);
        } else {
            self.push_node(TimelineNode::Assistant {
                text: text.to_owned(),
            });
        }
    }

    fn start_tool(&mut self, name: String, detail: Option<String>) {
        if let Some(idx) = self.active_tool_idx.take() {
            if let Some(TimelineNode::Tool {
                status,
                finished_at,
                ..
            }) = self.nodes.get_mut(idx)
            {
                *status = ToolStatus::Success;
                *finished_at = Some(Instant::now());
            }
        }

        self.push_node(TimelineNode::Tool {
            name,
            detail,
            status: ToolStatus::Running,
            started_at: Instant::now(),
            finished_at: None,
        });
        self.active_tool_idx = Some(self.nodes.len() - 1);
    }

    fn update_tool(&mut self, name: String, detail: Option<String>) {
        if let Some(idx) = self.find_running_tool_idx(&name) {
            if let Some(TimelineNode::Tool {
                detail: existing, ..
            }) = self.nodes.get_mut(idx)
            {
                if detail.as_ref().is_some_and(|d| !d.trim().is_empty()) {
                    *existing = detail;
                }
            }
            self.active_tool_idx = Some(idx);
            return;
        }

        self.start_tool(name, detail);
    }

    fn finish_tool(&mut self, name: String, detail: Option<String>, success: Option<bool>) {
        if let Some(idx) = self.find_running_tool_idx(&name) {
            if let Some(TimelineNode::Tool {
                detail: existing,
                status,
                finished_at,
                ..
            }) = self.nodes.get_mut(idx)
            {
                if detail.as_ref().is_some_and(|d| !d.trim().is_empty()) {
                    *existing = detail;
                }
                *status = match success {
                    Some(false) => ToolStatus::Failed,
                    _ => ToolStatus::Success,
                };
                *finished_at = Some(Instant::now());
            }
            self.active_tool_idx = None;
            return;
        }

        self.push_node(TimelineNode::Tool {
            name,
            detail,
            status: match success {
                Some(false) => ToolStatus::Failed,
                _ => ToolStatus::Success,
            },
            started_at: Instant::now(),
            finished_at: Some(Instant::now()),
        });
    }

    fn find_running_tool_idx(&self, name: &str) -> Option<usize> {
        self.nodes.iter().enumerate().rev().find_map(|(idx, node)| {
            let TimelineNode::Tool {
                name: node_name,
                status,
                ..
            } = node
            else {
                return None;
            };

            if *status == ToolStatus::Running && node_name == name {
                Some(idx)
            } else {
                None
            }
        })
    }

    fn push_done_if_missing(&mut self, text: String) {
        if self
            .nodes
            .last()
            .is_some_and(|node| matches!(node, TimelineNode::Done { .. }))
        {
            return;
        }
        self.push_node(TimelineNode::Done { text });
    }

    fn push_node(&mut self, node: TimelineNode) {
        self.nodes.push(node);
        self.prune();
    }

    fn prune(&mut self) {
        while self.nodes.len() > self.max_nodes || self.text_char_count() > self.max_text_chars {
            if self.nodes.is_empty() {
                break;
            }
            self.nodes.remove(0);
            if let Some(idx) = self.active_tool_idx {
                self.active_tool_idx = idx.checked_sub(1);
            }
        }
    }

    fn text_char_count(&self) -> usize {
        self.nodes
            .iter()
            .map(|node| match node {
                TimelineNode::Thinking { text, .. } => text.len(),
                TimelineNode::Assistant { text } => text.len(),
                TimelineNode::Tool { name, detail, .. } => {
                    name.len() + detail.as_ref().map_or(0, String::len)
                }
                TimelineNode::System { text } => text.len(),
                TimelineNode::Error { text } => text.len(),
                TimelineNode::Done { text } => text.len(),
            })
            .sum()
    }

    fn activity_summary_line(&self) -> Option<String> {
        let mut explored = 0usize;
        let mut edited = 0usize;
        let mut ran = 0usize;
        let mut other = 0usize;
        let mut thought_for = Duration::default();

        for node in &self.nodes {
            match node {
                TimelineNode::Tool { name, detail, .. } => {
                    match classify_tool_kind(name, detail.as_deref()) {
                        ActivityKind::Explore => explored += 1,
                        ActivityKind::Edit => edited += 1,
                        ActivityKind::Run => ran += 1,
                        ActivityKind::Other => other += 1,
                    }
                }
                TimelineNode::Thinking {
                    started_at,
                    finished_at,
                    ..
                } => {
                    thought_for =
                        thought_for.saturating_add(finished_at.duration_since(*started_at));
                }
                _ => {}
            }
        }

        if let Some(active) = &self.active_thinking {
            thought_for = thought_for.saturating_add(active.started_at.elapsed());
        }

        let mut parts = Vec::new();
        if !thought_for.is_zero() {
            parts.push(format!("thought for {}", format_duration(thought_for)));
        }
        if explored > 0 {
            parts.push(format!("explored {explored}"));
        }
        if edited > 0 {
            parts.push(format!("edited {edited}"));
        }
        if ran > 0 {
            parts.push(format!("ran {ran}"));
        }
        if other > 0 {
            let label = if other == 1 { "tool" } else { "tools" };
            parts.push(format!("used {other} {label}"));
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join(", "))
        }
    }
}

fn summarize_tool(name: &str, detail: Option<&str>) -> ToolDigest {
    let kind = classify_tool_kind(name, detail);

    let mut subject = None;
    let mut change_summary = None;
    let mut detail_hint = None;

    if let Some(detail) = detail.map(str::trim).filter(|d| !d.is_empty()) {
        if let Ok(json) = serde_json::from_str::<Value>(detail) {
            subject = extract_subject_from_json(&json, kind);
            change_summary = extract_change_summary_from_json(&json);
        } else {
            if matches!(kind, ActivityKind::Edit) {
                change_summary = summarize_patch(detail);
            }

            if subject.is_none() && !looks_like_structured_blob(detail) {
                detail_hint = Some(compact_inline(detail, 88));
            }
        }
    }

    ToolDigest {
        kind,
        subject,
        change_summary,
        detail_hint,
    }
}

fn classify_tool_kind(name: &str, detail: Option<&str>) -> ActivityKind {
    let lower = name.to_ascii_lowercase();

    if has_any(
        &lower,
        &[
            "edit",
            "apply_patch",
            "write",
            "replace",
            "create_file",
            "delete_file",
            "insert",
            "update_file",
        ],
    ) {
        return ActivityKind::Edit;
    }

    if has_any(
        &lower,
        &[
            "exec", "shell", "command", "terminal", "bash", "zsh", "sh", "run",
        ],
    ) {
        return ActivityKind::Run;
    }

    if has_any(
        &lower,
        &[
            "read",
            "open",
            "view",
            "list",
            "ls",
            "find",
            "search",
            "grep",
            "rg",
            "glob",
            "query",
            "fetch",
            "screenshot",
            "click",
        ],
    ) {
        return ActivityKind::Explore;
    }

    if let Some(detail) = detail {
        let detail_lower = detail.to_ascii_lowercase();
        if detail_lower.contains("\"cmd\"") || detail_lower.contains("\"command\"") {
            return ActivityKind::Run;
        }
        if detail_lower.contains("\"patch\"")
            || detail_lower.contains("\"diff\"")
            || detail_lower.contains("@@")
        {
            return ActivityKind::Edit;
        }
    }

    ActivityKind::Other
}

fn has_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn extract_subject_from_json(value: &Value, kind: ActivityKind) -> Option<String> {
    let path = first_string(
        value,
        &[
            "/path",
            "/file",
            "/file_path",
            "/filepath",
            "/filename",
            "/relative_path",
            "/target",
            "/args/path",
            "/args/file",
            "/args/file_path",
            "/args/filename",
        ],
    );
    let cmd = first_string(
        value,
        &[
            "/cmd",
            "/command",
            "/args/cmd",
            "/args/command",
            "/input/cmd",
        ],
    );
    let query = first_string(
        value,
        &[
            "/pattern",
            "/query",
            "/q",
            "/args/pattern",
            "/args/query",
            "/search_query/0/q",
        ],
    );

    match kind {
        ActivityKind::Edit | ActivityKind::Explore => {
            if let Some(path) = path {
                return Some(shorten_path(&path, 42));
            }
            if let Some(query) = query {
                return Some(format!("query `{}`", compact_inline(&query, 28)));
            }
        }
        ActivityKind::Run => {
            if let Some(cmd) = cmd {
                return Some(format!("`{}`", compact_inline(&cmd, 52)));
            }
        }
        ActivityKind::Other => {
            if let Some(path) = path {
                return Some(shorten_path(&path, 42));
            }
            if let Some(cmd) = cmd {
                return Some(format!("`{}`", compact_inline(&cmd, 40)));
            }
        }
    }

    None
}

fn extract_change_summary_from_json(value: &Value) -> Option<String> {
    let adds = first_u64(
        value,
        &[
            "/additions",
            "/added",
            "/lines_added",
            "/stats/additions",
            "/result/additions",
        ],
    );
    let dels = first_u64(
        value,
        &[
            "/deletions",
            "/removed",
            "/lines_removed",
            "/stats/deletions",
            "/result/deletions",
        ],
    );

    if adds.is_some() || dels.is_some() {
        return Some(format_change_summary(
            adds.unwrap_or(0) as usize,
            dels.unwrap_or(0) as usize,
        ));
    }

    first_string(value, &["/patch", "/diff", "/args/patch", "/args/diff"])
        .and_then(|text| summarize_patch(&text))
}

fn summarize_patch(text: &str) -> Option<String> {
    let (mut added, mut removed) = (0usize, 0usize);

    for line in text.lines() {
        if line.starts_with("+++") || line.starts_with("---") {
            continue;
        }
        if line.starts_with('+') {
            added += 1;
        } else if line.starts_with('-') {
            removed += 1;
        }
    }

    if added == 0 && removed == 0 {
        None
    } else {
        Some(format_change_summary(added, removed))
    }
}

fn format_change_summary(added: usize, removed: usize) -> String {
    format!("(+{added} -{removed})")
}

fn first_string(value: &Value, pointers: &[&str]) -> Option<String> {
    pointers
        .iter()
        .find_map(|ptr| value.pointer(ptr).and_then(Value::as_str))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

fn first_u64(value: &Value, pointers: &[&str]) -> Option<u64> {
    pointers
        .iter()
        .find_map(|ptr| value.pointer(ptr).and_then(Value::as_u64))
}

fn shorten_path(path: &str, max_chars: usize) -> String {
    let compact = path.split_whitespace().collect::<Vec<_>>().join(" ");
    let char_count = compact.chars().count();
    if char_count <= max_chars {
        return compact;
    }

    let tail = compact
        .chars()
        .skip(char_count.saturating_sub(max_chars.saturating_sub(1)))
        .collect::<String>();
    format!("…{tail}")
}

fn compact_inline(text: &str, max_chars: usize) -> String {
    let mut compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let char_count = compact.chars().count();
    if char_count <= max_chars {
        return compact;
    }

    compact = compact.chars().take(max_chars).collect::<String>();
    compact.push('…');
    compact
}

fn needs_separator(existing: &str, next: &str) -> bool {
    let Some(last) = existing.chars().last() else {
        return false;
    };
    if last.is_whitespace() {
        return false;
    }

    let first = next.chars().next().unwrap_or(' ');
    !(first.is_whitespace() || ",.;:!?)]}".contains(first))
}

fn format_duration(duration: Duration) -> String {
    if duration.as_secs() >= 60 {
        let minutes = duration.as_secs() / 60;
        let seconds = duration.as_secs() % 60;
        return format!("{minutes}m {seconds:02}s");
    }
    if duration.as_secs() >= 1 {
        return format!("{}s", duration.as_secs());
    }

    let millis = duration.as_millis().max(1);
    format!("{millis}ms")
}

fn looks_like_structured_blob(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with('{') || trimmed.starts_with('[')
}

#[cfg(test)]
mod tests {
    use super::{AgentOutputMode, AgentTimeline};
    use crate::agent::stream::AgentStreamEvent;

    #[test]
    fn toggles_view_mode() {
        assert_eq!(AgentOutputMode::Ui.toggle(), AgentOutputMode::Raw);
        assert_eq!(AgentOutputMode::Raw.toggle(), AgentOutputMode::Ui);
    }

    #[test]
    fn keeps_thinking_trace_after_thinking_finishes() {
        let mut timeline = AgentTimeline::new(100, 100, 10_000);
        timeline.apply_event(AgentStreamEvent::ThinkingDelta("planning".to_owned()));
        let active_lines = timeline.ui_lines(true).join("\n");
        assert!(active_lines.contains("Thinking for"));
        assert!(active_lines.contains("planning"));

        timeline.apply_event(AgentStreamEvent::AssistantDelta("done".to_owned()));
        let lines = timeline.ui_lines(true).join("\n");
        assert!(!lines.contains("Thinking for"));
        assert!(lines.contains("thought for"));
        assert!(lines.contains("planning"));
        assert!(lines.contains("🤖 Assistant"));
    }

    #[test]
    fn renders_compact_edit_activity() {
        let mut timeline = AgentTimeline::new(100, 100, 10_000);
        timeline.apply_event(AgentStreamEvent::ToolStart {
            name: "apply_patch".to_owned(),
            detail: Some(
                r#"{"path":"src/app.rs","patch":"@@\n-old\n+new\n+plus\n-removed"}"#.to_owned(),
            ),
        });
        timeline.apply_event(AgentStreamEvent::ToolEnd {
            name: "apply_patch".to_owned(),
            detail: None,
            success: Some(true),
        });

        let lines = timeline.ui_lines(true).join("\n");
        assert!(lines.contains("edited 1"));
        assert!(lines.contains("edited via `apply_patch` src/app.rs (+2 -2)"));
    }

    #[test]
    fn keeps_raw_lines_bounded() {
        let mut timeline = AgentTimeline::new(2, 100, 10_000);
        timeline.push_raw_line("one".to_owned());
        timeline.push_raw_line("two".to_owned());
        timeline.push_raw_line("three".to_owned());
        let raw: Vec<String> = timeline.raw_logs().iter().cloned().collect();
        assert_eq!(raw, vec!["two".to_owned(), "three".to_owned()]);
    }

    #[test]
    fn mark_complete_success_adds_done_node() {
        let mut timeline = AgentTimeline::new(100, 100, 10_000);
        timeline.apply_event(AgentStreamEvent::Info("started".to_owned()));
        timeline.mark_complete(true, None);
        let lines = timeline.ui_lines(true).join("\n");
        assert!(lines.contains("Completed") || lines.contains("✅"));
    }

    #[test]
    fn mark_complete_failure_adds_error() {
        let mut timeline = AgentTimeline::new(100, 100, 10_000);
        timeline.mark_complete(false, Some("something broke"));
        let lines = timeline.ui_lines(true).join("\n");
        assert!(lines.contains("something broke"));
    }

    #[test]
    fn done_event_is_deduplicated() {
        let mut timeline = AgentTimeline::new(100, 100, 10_000);
        timeline.apply_event(AgentStreamEvent::Done);
        timeline.apply_event(AgentStreamEvent::Done);
        let lines = timeline.ui_lines(true);
        let done_count = lines.iter().filter(|l| l.contains("Completed")).count();
        assert_eq!(done_count, 1);
    }

    #[test]
    fn error_event_shows_in_ui() {
        let mut timeline = AgentTimeline::new(100, 100, 10_000);
        timeline.apply_event(AgentStreamEvent::Error("oops".to_owned()));
        let lines = timeline.ui_lines(true).join("\n");
        assert!(lines.contains("oops"));
    }
}
