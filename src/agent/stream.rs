use serde_json::Value;

#[derive(Debug, Clone)]
pub enum StreamChunk {
    Stdout(String),
    Stderr(String),
    System(String),
}

impl StreamChunk {
    pub fn raw_line(&self) -> String {
        match self {
            StreamChunk::Stdout(line) => line.clone(),
            StreamChunk::Stderr(line) => format!("stderr: {line}"),
            StreamChunk::System(line) => format!("system: {line}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentStreamEvent {
    ThinkingDelta(String),
    ThinkingDone,
    AssistantDelta(String),
    ToolStart {
        name: String,
        detail: Option<String>,
    },
    ToolUpdate {
        name: String,
        detail: Option<String>,
    },
    ToolEnd {
        name: String,
        detail: Option<String>,
        success: Option<bool>,
    },
    Error(String),
    Info(String),
    Done,
}

pub fn parse_stream_chunk(chunk: &StreamChunk) -> Option<AgentStreamEvent> {
    match chunk {
        StreamChunk::Stdout(line) => parse_stdout_line(line),
        StreamChunk::Stderr(line) => Some(AgentStreamEvent::Error(line.clone())),
        StreamChunk::System(line) => Some(AgentStreamEvent::Info(line.clone())),
    }
}

fn parse_stdout_line(line: &str) -> Option<AgentStreamEvent> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let Some(value) = parse_json_fragment(trimmed) else {
        return Some(AgentStreamEvent::Info(trimmed.to_owned()));
    };

    let event_type = get_str(&value, "type")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let subtype = get_str(&value, "subtype")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let text = extract_text(&value).map(|t| t.trim().to_owned());
    let text = text.filter(|t| !t.is_empty());

    if event_type == "thinking" {
        if is_done_marker(&subtype) {
            return Some(AgentStreamEvent::ThinkingDone);
        }
        if let Some(text) = text {
            return Some(AgentStreamEvent::ThinkingDelta(text));
        }
        return None;
    }

    if event_type == "error" || subtype == "error" {
        if let Some(text) = text {
            return Some(AgentStreamEvent::Error(text));
        }
        return Some(AgentStreamEvent::Error("agent stream error".to_owned()));
    }

    if is_tool_event(&event_type, &subtype, &value) {
        let name =
            extract_tool_name(&value).unwrap_or_else(|| fallback_tool_name(&event_type, &subtype));
        let detail = text.or_else(|| extract_tool_detail(&value));
        let success = extract_success(&value);

        if is_start_marker(&subtype) {
            return Some(AgentStreamEvent::ToolStart { name, detail });
        }
        if is_done_marker(&subtype) || success.is_some() {
            return Some(AgentStreamEvent::ToolEnd {
                name,
                detail,
                success,
            });
        }
        return Some(AgentStreamEvent::ToolUpdate { name, detail });
    }

    if is_done_marker(&event_type) || is_done_marker(&subtype) {
        return Some(AgentStreamEvent::Done);
    }

    if let Some(text) = text {
        return Some(AgentStreamEvent::AssistantDelta(text));
    }

    None
}

fn parse_json_fragment(input: &str) -> Option<Value> {
    if let Ok(v) = serde_json::from_str::<Value>(input) {
        return Some(v);
    }

    let start = input.find('{')?;
    let end = input.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str::<Value>(&input[start..=end]).ok()
}

fn get_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

fn extract_text(value: &Value) -> Option<String> {
    if let Some(text) = get_str(value, "text") {
        return Some(text.to_owned());
    }
    if let Some(text) = get_str(value, "delta") {
        return Some(text.to_owned());
    }
    if let Some(text) = get_str(value, "content") {
        return Some(text.to_owned());
    }
    if let Some(content) = value.get("content").and_then(|v| v.as_array()) {
        let joined = content
            .iter()
            .filter_map(|item| {
                item.get("text")
                    .and_then(|v| v.as_str())
                    .or_else(|| item.as_str())
            })
            .collect::<Vec<_>>()
            .join("");
        if !joined.is_empty() {
            return Some(joined);
        }
    }
    if let Some(message) = value.get("message") {
        if let Some(text) = message.get("text").and_then(|v| v.as_str()) {
            return Some(text.to_owned());
        }
        if let Some(content) = message.get("content").and_then(|v| v.as_str()) {
            return Some(content.to_owned());
        }
    }
    None
}

fn is_done_marker(value: &str) -> bool {
    matches!(
        value,
        "done" | "end" | "ended" | "complete" | "completed" | "stop" | "stopped"
    )
}

fn is_start_marker(value: &str) -> bool {
    matches!(
        value,
        "start" | "started" | "begin" | "began" | "call" | "invoked"
    )
}

fn is_tool_event(event_type: &str, subtype: &str, value: &Value) -> bool {
    event_type.contains("tool")
        || event_type.contains("function")
        || subtype.contains("tool")
        || subtype.contains("function")
        || value.get("tool_name").is_some()
        || value.get("tool").is_some()
        || value.get("tool_call").is_some()
        || value.get("tool_calls").is_some()
        || value.get("function").is_some()
        || value.get("call").is_some()
}

fn extract_tool_name(value: &Value) -> Option<String> {
    first_string(
        value,
        &[
            "/tool_name",
            "/toolName",
            "/tool",
            "/function",
            "/name",
            "/tool/name",
            "/function/name",
            "/tool_call/tool/name",
            "/tool_call/function",
            "/tool_call/function/name",
            "/tool_call/name",
            "/toolCall/name",
            "/call/tool_name",
            "/call/tool/name",
            "/call/function",
            "/call/function/name",
            "/call/name",
            "/data/tool_name",
            "/data/tool/name",
            "/data/function/name",
            "/data/function",
            "/tool_calls/0/name",
            "/tool_calls/0/tool/name",
            "/tool_calls/0/function/name",
            "/delta/tool_calls/0/name",
            "/delta/tool_calls/0/tool/name",
            "/delta/tool_calls/0/function/name",
            "/message/tool_calls/0/name",
            "/message/tool_calls/0/tool/name",
            "/message/tool_calls/0/function/name",
        ],
    )
    .or_else(|| find_first_key_string(value, &["tool_name", "toolName"]))
    .or_else(|| find_first_key_string(value, &["function_name", "function"]))
    .or_else(|| find_first_key_string(value, &["tool", "name"]))
    .and_then(normalize_tool_name)
}

fn extract_tool_detail(value: &Value) -> Option<String> {
    if let Some(summary) = first_string(
        value,
        &[
            "/summary",
            "/message",
            "/detail",
            "/args",
            "/input",
            "/arguments",
            "/function/arguments",
            "/tool_call/arguments",
            "/tool_calls/0/arguments",
            "/tool_calls/0/function/arguments",
            "/delta/tool_calls/0/arguments",
            "/delta/tool_calls/0/function/arguments",
            "/message/tool_calls/0/arguments",
            "/message/tool_calls/0/function/arguments",
        ],
    ) {
        return Some(summary);
    }

    value
        .get("args")
        .or_else(|| value.get("input"))
        .or_else(|| value.get("data"))
        .map(|v| v.to_string())
}

fn extract_success(value: &Value) -> Option<bool> {
    if let Some(success) = value.get("success").and_then(|v| v.as_bool()) {
        return Some(success);
    }
    if let Some(status) = get_str(value, "status") {
        match status.to_ascii_lowercase().as_str() {
            "ok" | "success" | "completed" => return Some(true),
            "error" | "failed" | "failure" => return Some(false),
            _ => {}
        }
    }
    None
}

fn first_string(value: &Value, pointers: &[&str]) -> Option<String> {
    pointers
        .iter()
        .find_map(|ptr| value.pointer(ptr).and_then(|v| v.as_str()))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

fn find_first_key_string(value: &Value, keys: &[&str]) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(s) = map
                    .get(*key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    return Some(s.to_owned());
                }
            }
            map.values()
                .find_map(|nested| find_first_key_string(nested, keys))
        }
        Value::Array(items) => items
            .iter()
            .find_map(|item| find_first_key_string(item, keys)),
        _ => None,
    }
}

fn normalize_tool_name(name: String) -> Option<String> {
    let cleaned = name.trim().trim_matches('"').to_owned();
    if cleaned.is_empty()
        || cleaned.eq_ignore_ascii_case("tool")
        || cleaned.eq_ignore_ascii_case("function")
    {
        None
    } else {
        Some(cleaned)
    }
}

fn fallback_tool_name(event_type: &str, subtype: &str) -> String {
    for raw in [event_type, subtype] {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_done_marker(trimmed) || is_start_marker(trimmed) {
            continue;
        }
        if matches!(trimmed, "tool" | "tools" | "function" | "call") {
            continue;
        }
        return trimmed.replace('-', "_");
    }
    "unknown_tool".to_owned()
}

#[cfg(test)]
mod tests {
    use super::{parse_stream_chunk, AgentStreamEvent, StreamChunk};

    #[test]
    fn parses_thinking_delta() {
        let chunk = StreamChunk::Stdout(
            r#"{"type":"thinking","subtype":"delta","text":"investigating"}"#.to_owned(),
        );
        let event = parse_stream_chunk(&chunk);
        assert_eq!(
            event,
            Some(AgentStreamEvent::ThinkingDelta("investigating".to_owned()))
        );
    }

    #[test]
    fn parses_tool_start_and_end() {
        let start = StreamChunk::Stdout(
            r#"{"type":"tool","subtype":"start","tool_name":"edit_file","text":"editing"}"#
                .to_owned(),
        );
        let end = StreamChunk::Stdout(
            r#"{"type":"tool","subtype":"complete","tool_name":"edit_file","success":true}"#
                .to_owned(),
        );

        assert_eq!(
            parse_stream_chunk(&start),
            Some(AgentStreamEvent::ToolStart {
                name: "edit_file".to_owned(),
                detail: Some("editing".to_owned()),
            })
        );
        assert_eq!(
            parse_stream_chunk(&end),
            Some(AgentStreamEvent::ToolEnd {
                name: "edit_file".to_owned(),
                detail: None,
                success: Some(true),
            })
        );
    }

    #[test]
    fn parses_nested_tool_call_name() {
        let nested = StreamChunk::Stdout(
            r#"{"type":"tool","subtype":"start","tool_calls":[{"function":{"name":"exec_command","arguments":"{\"cmd\":\"rg --files\"}"}}]}"#
                .to_owned(),
        );
        assert_eq!(
            parse_stream_chunk(&nested),
            Some(AgentStreamEvent::ToolStart {
                name: "exec_command".to_owned(),
                detail: Some("{\"cmd\":\"rg --files\"}".to_owned()),
            })
        );
    }

    #[test]
    fn parses_function_name_when_emitted_as_string() {
        let chunk = StreamChunk::Stdout(
            r#"{"type":"function_call","subtype":"start","function":"exec_command","arguments":"{\"cmd\":\"pwd\"}"}"#
                .to_owned(),
        );
        assert_eq!(
            parse_stream_chunk(&chunk),
            Some(AgentStreamEvent::ToolStart {
                name: "exec_command".to_owned(),
                detail: Some("{\"cmd\":\"pwd\"}".to_owned()),
            })
        );
    }

    #[test]
    fn treats_non_json_as_info() {
        let chunk = StreamChunk::Stdout("plain text chunk".to_owned());
        let event = parse_stream_chunk(&chunk);
        assert_eq!(
            event,
            Some(AgentStreamEvent::Info("plain text chunk".to_owned()))
        );
    }
}
