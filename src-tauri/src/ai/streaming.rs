use serde::{Deserialize, Serialize};

// --- Public message types ---

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: FunctionCall,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct AiStreamEvent {
    pub session_id: String,
    pub event_type: String,
    pub content: String,
}

// --- SSE stream parsing helpers ---

#[derive(Deserialize, Debug)]
pub(super) struct SseDelta {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<SseDeltaToolCall>>,
}

#[derive(Deserialize, Debug)]
pub(super) struct SseDeltaToolCall {
    pub index: Option<usize>,
    pub id: Option<String>,
    #[allow(dead_code)]
    pub r#type: Option<String>,
    pub function: Option<SseDeltaFunction>,
}

#[derive(Deserialize, Debug)]
pub(super) struct SseDeltaFunction {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

#[derive(Deserialize, Debug)]
pub(super) struct SseChoice {
    pub delta: Option<SseDelta>,
    pub finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
pub(super) struct SseChunk {
    pub choices: Option<Vec<SseChoice>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    // --- ChatMessage tests ---

    #[test]
    fn chat_message_serialize_minimal() {
        let msg = ChatMessage {
            role: "user".to_string(),
            content: Some("Hello".to_string()),
            tool_calls: None,
            tool_call_id: None,
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["role"], "user");
        assert_eq!(json["content"], "Hello");
        // Optional None fields should be skipped
        assert!(json.get("tool_calls").is_none());
        assert!(json.get("tool_call_id").is_none());
    }

    #[test]
    fn chat_message_serialize_with_tool_calls() {
        let msg = ChatMessage {
            role: "assistant".to_string(),
            content: None,
            tool_calls: Some(vec![ToolCall {
                id: "call_1".to_string(),
                r#type: "function".to_string(),
                function: FunctionCall {
                    name: "search".to_string(),
                    arguments: r#"{"q":"test"}"#.to_string(),
                },
            }]),
            tool_call_id: None,
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert!(json.get("content").is_none());
        let tc = &json["tool_calls"][0];
        assert_eq!(tc["id"], "call_1");
        assert_eq!(tc["type"], "function");
        assert_eq!(tc["function"]["name"], "search");
    }

    #[test]
    fn chat_message_deserialize_tool_role() {
        let json = r#"{
            "role": "tool",
            "content": "result data",
            "tool_call_id": "call_42"
        }"#;
        let msg: ChatMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.role, "tool");
        assert_eq!(msg.content.as_deref(), Some("result data"));
        assert_eq!(msg.tool_call_id.as_deref(), Some("call_42"));
        assert!(msg.tool_calls.is_none());
    }

    #[test]
    fn chat_message_roundtrip() {
        let original = ChatMessage {
            role: "assistant".to_string(),
            content: Some("Here is the answer".to_string()),
            tool_calls: Some(vec![
                ToolCall {
                    id: "tc_1".to_string(),
                    r#type: "function".to_string(),
                    function: FunctionCall {
                        name: "read_file".to_string(),
                        arguments: r#"{"path":"/tmp/test.txt"}"#.to_string(),
                    },
                },
                ToolCall {
                    id: "tc_2".to_string(),
                    r#type: "function".to_string(),
                    function: FunctionCall {
                        name: "write_file".to_string(),
                        arguments: r#"{"path":"/tmp/out.txt","content":"hello"}"#.to_string(),
                    },
                },
            ]),
            tool_call_id: None,
        };
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: ChatMessage = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.role, "assistant");
        assert_eq!(deserialized.content.as_deref(), Some("Here is the answer"));
        assert_eq!(deserialized.tool_calls.as_ref().unwrap().len(), 2);
        assert_eq!(deserialized.tool_calls.as_ref().unwrap()[1].function.name, "write_file");
    }

    // --- ToolCall / FunctionCall tests ---

    #[test]
    fn tool_call_deserialize() {
        let json = r#"{
            "id": "call_abc",
            "type": "function",
            "function": {
                "name": "grep_files",
                "arguments": "{\"pattern\":\"TODO\"}"
            }
        }"#;
        let tc: ToolCall = serde_json::from_str(json).unwrap();
        assert_eq!(tc.id, "call_abc");
        assert_eq!(tc.r#type, "function");
        assert_eq!(tc.function.name, "grep_files");
        assert_eq!(tc.function.arguments, r#"{"pattern":"TODO"}"#);
    }

    #[test]
    fn function_call_serialize() {
        let fc = FunctionCall {
            name: "run_python".to_string(),
            arguments: r#"{"code":"print(1+1)"}"#.to_string(),
        };
        let json = serde_json::to_value(&fc).unwrap();
        assert_eq!(json["name"], "run_python");
        assert_eq!(json["arguments"], r#"{"code":"print(1+1)"}"#);
    }

    // --- AiStreamEvent tests ---

    #[test]
    fn ai_stream_event_serialize() {
        let event = AiStreamEvent {
            session_id: "sess_123".to_string(),
            event_type: "content".to_string(),
            content: "Hello world".to_string(),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["session_id"], "sess_123");
        assert_eq!(json["event_type"], "content");
        assert_eq!(json["content"], "Hello world");
    }

    // --- SseChunk tests ---

    #[test]
    fn sse_chunk_with_content_delta() {
        let json = r#"{
            "choices": [{
                "delta": {
                    "content": "Hello"
                },
                "finish_reason": null
            }]
        }"#;
        let chunk: SseChunk = serde_json::from_str(json).unwrap();
        let choices = chunk.choices.unwrap();
        assert_eq!(choices.len(), 1);
        let choice = &choices[0];
        let delta = choice.delta.as_ref().unwrap();
        assert_eq!(delta.content.as_deref(), Some("Hello"));
        assert!(delta.tool_calls.is_none());
        assert!(choice.finish_reason.is_none());
    }

    #[test]
    fn sse_chunk_with_finish_reason() {
        let json = r#"{
            "choices": [{
                "delta": {},
                "finish_reason": "stop"
            }]
        }"#;
        let chunk: SseChunk = serde_json::from_str(json).unwrap();
        let choices = chunk.choices.unwrap();
        assert_eq!(choices[0].finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn sse_chunk_with_tool_calls_delta() {
        let json = r#"{
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "id": "call_xyz",
                        "type": "function",
                        "function": {
                            "name": "search_files",
                            "arguments": "{\"q\":"
                        }
                    }]
                },
                "finish_reason": null
            }]
        }"#;
        let chunk: SseChunk = serde_json::from_str(json).unwrap();
        let choices = chunk.choices.unwrap();
        let delta = choices[0].delta.as_ref().unwrap();
        let tc = &delta.tool_calls.as_ref().unwrap()[0];
        assert_eq!(tc.index, Some(0));
        assert_eq!(tc.id.as_deref(), Some("call_xyz"));
        let func = tc.function.as_ref().unwrap();
        assert_eq!(func.name.as_deref(), Some("search_files"));
        assert_eq!(func.arguments.as_deref(), Some("{\"q\":"));
    }

    #[test]
    fn sse_chunk_with_missing_optional_fields() {
        let json = r#"{}"#;
        let chunk: SseChunk = serde_json::from_str(json).unwrap();
        assert!(chunk.choices.is_none());
    }

    #[test]
    fn sse_chunk_empty_choices() {
        let json = r#"{"choices": []}"#;
        let chunk: SseChunk = serde_json::from_str(json).unwrap();
        assert!(chunk.choices.unwrap().is_empty());
    }

    #[test]
    fn sse_delta_tool_call_partial_no_function() {
        let json = r#"{
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "function": {
                            "arguments": "test_arg"
                        }
                    }]
                },
                "finish_reason": null
            }]
        }"#;
        let chunk: SseChunk = serde_json::from_str(json).unwrap();
        let choices = chunk.choices.unwrap();
        let tc = &choices[0].delta.as_ref().unwrap().tool_calls.as_ref().unwrap()[0];
        assert!(tc.id.is_none());
        assert!(tc.r#type.is_none());
        assert!(tc.function.as_ref().unwrap().name.is_none());
        assert_eq!(tc.function.as_ref().unwrap().arguments.as_deref(), Some("test_arg"));
    }
}
