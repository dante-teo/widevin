mod helpers;

use prost::Message;
use widevin::{
    DevinContentPart, DevinError, DevinMessage, DevinStopReason, DevinStreamEvent, FetchResponse,
    StreamDevinChatOptions, build_chat_message_prompts, encode_connect_frame,
    fetch_response_from_bytes, proto, stream_devin_chat,
};

fn auth_response(custom_url: &str) -> FetchResponse {
    fetch_response_from_bytes(
        200,
        "OK",
        proto::GetUserJwtResponse {
            user_jwt: "jwt-1".into(),
            custom_api_server_url: custom_url.into(),
        }
        .encode_to_vec(),
    )
}

#[tokio::test]
async fn tool_arguments_accumulate_incremental_and_cumulative_updates_per_id() {
    let cumulative = (0..260)
        .map(|index| format!("{{}}{}", " ".repeat(index)))
        .collect::<Vec<_>>();
    let calls = [
        vec![
            proto::ChatToolCall {
                id: "call-a".into(),
                name: "first".into(),
                arguments_json: cumulative[0].clone(),
            },
            proto::ChatToolCall {
                id: "call-b".into(),
                name: "second".into(),
                arguments_json: r#"{"id":"b"}"#.into(),
            },
        ],
        cumulative
            .iter()
            .skip(1)
            .map(|arguments_json| proto::ChatToolCall {
                id: "call-a".into(),
                name: String::new(),
                arguments_json: arguments_json.clone(),
            })
            .collect(),
    ]
    .concat();
    let frame = message_frame(&proto::GetChatMessageResponse {
        delta_tool_calls: calls,
        ..Default::default()
    });
    let (fetch, _) = helpers::fetch_sequence(vec![
        Ok(auth_response("")),
        Ok(helpers::chunked_response(vec![frame])),
    ]);
    let events = helpers::collect_events(stream_devin_chat(StreamDevinChatOptions {
        model: "model-a".into(),
        fetch: Some(fetch),
        ..Default::default()
    }))
    .await
    .expect("events");
    let first_deltas = events
        .iter()
        .filter_map(|event| match event {
            DevinStreamEvent::ToolCallDelta {
                id,
                delta,
                arguments,
            } if id == "call-a" => Some((delta, arguments)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        first_deltas
            .iter()
            .map(|(delta, _)| delta.as_str())
            .collect::<String>(),
        *cumulative.last().expect("last cumulative value")
    );
    assert_eq!(
        first_deltas
            .iter()
            .enumerate()
            .filter_map(|(index, (_, arguments))| arguments.as_ref().map(|_| index))
            .collect::<Vec<_>>(),
        vec![0, 256]
    );
    assert!(events.iter().any(|event| {
        matches!(
            event,
            DevinStreamEvent::ToolCallDelta {
                id,
                arguments: Some(arguments),
                ..
            } if id == "call-b" && arguments == &serde_json::json!({"id":"b"})
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            event,
            DevinStreamEvent::ToolCallEnd {
                id,
                arguments,
                ..
            } if id == "call-a" && arguments == &serde_json::json!({})
        )
    }));
}

#[tokio::test]
async fn chat_http_failure_after_auth_is_structured() {
    let (fetch, _) = helpers::fetch_sequence(vec![
        Ok(auth_response("")),
        Ok(fetch_response_from_bytes(
            429,
            "Rate Limited",
            b"nope".to_vec(),
        )),
    ]);
    let error = helpers::collect_events(stream_devin_chat(StreamDevinChatOptions {
        model: "model-a".into(),
        fetch: Some(fetch),
        ..Default::default()
    }))
    .await
    .expect_err("chat error");
    assert!(matches!(error, DevinError::Api { status: 429, .. }));
}

#[tokio::test]
async fn chat_rejects_a_missing_response_body() {
    let (fetch, _) = helpers::fetch_sequence(vec![
        Ok(auth_response("")),
        Ok(FetchResponse {
            status: 200,
            status_text: "OK".into(),
            body: None,
        }),
    ]);
    let error = helpers::collect_events(stream_devin_chat(StreamDevinChatOptions {
        model: "model-a".into(),
        fetch: Some(fetch),
        ..Default::default()
    }))
    .await
    .expect_err("missing body");
    assert!(matches!(error, DevinError::Protocol { .. }));
}

fn message_frame(response: &proto::GetChatMessageResponse) -> Vec<u8> {
    encode_connect_frame(&response.encode_to_vec(), true, false).expect("message frame")
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn chat_stream_matches_text_thinking_tools_usage_and_done_behavior() {
    let frames = vec![
        message_frame(&proto::GetChatMessageResponse {
            delta_thinking: "think".into(),
            ..Default::default()
        }),
        message_frame(&proto::GetChatMessageResponse {
            delta_text: "hi".into(),
            ..Default::default()
        }),
        message_frame(&proto::GetChatMessageResponse {
            delta_tool_calls: vec![proto::ChatToolCall {
                id: "call-1".into(),
                name: "search".into(),
                arguments_json: "{\"q\"".into(),
            }],
            ..Default::default()
        }),
        message_frame(&proto::GetChatMessageResponse {
            delta_tool_calls: vec![proto::ChatToolCall {
                id: "call-1".into(),
                arguments_json: "{\"q\":\"x\"}".into(),
                ..Default::default()
            }],
            usage: Some(proto::ModelUsageStats {
                input_tokens: 2,
                output_tokens: 3,
                cache_read_tokens: 4,
                cache_write_tokens: 5,
            }),
            ..Default::default()
        }),
        encode_connect_frame(b"{}", false, true).expect("trailer"),
    ];
    let (fetch, requests) = helpers::fetch_sequence(vec![
        Ok(auth_response("https://chat.example/")),
        Ok(helpers::chunked_response(frames)),
    ]);
    let events = helpers::collect_events(stream_devin_chat(StreamDevinChatOptions {
        token: Some("raw".into()),
        base_url: "https://server.example".into(),
        model: "model-a".into(),
        messages: vec![DevinMessage::User {
            content: vec![DevinContentPart::Text {
                text: "hello".into(),
            }],
        }],
        uuid: Some(std::sync::Arc::new(|| "uuid-1".into())),
        fetch: Some(fetch),
        ..Default::default()
    }))
    .await
    .expect("events");
    assert_eq!(
        events,
        vec![
            DevinStreamEvent::ThinkingDelta {
                delta: "think".into(),
                signature: None
            },
            DevinStreamEvent::TextDelta { delta: "hi".into() },
            DevinStreamEvent::ToolCallStart {
                id: "call-1".into(),
                name: "search".into()
            },
            DevinStreamEvent::ToolCallDelta {
                id: "call-1".into(),
                delta: "{\"q\"".into(),
                arguments: None
            },
            DevinStreamEvent::ToolCallDelta {
                id: "call-1".into(),
                delta: ":\"x\"}".into(),
                arguments: None
            },
            DevinStreamEvent::Usage {
                input_tokens: 2,
                output_tokens: 3,
                cache_read_tokens: 4,
                cache_write_tokens: 5
            },
            DevinStreamEvent::ToolCallEnd {
                id: "call-1".into(),
                name: "search".into(),
                arguments: serde_json::json!({"q":"x"})
            },
            DevinStreamEvent::Done {
                reason: DevinStopReason::ToolUse
            }
        ]
    );
    let requests = requests.lock().expect("requests");
    assert!(
        requests[0]
            .url
            .ends_with("/exa.auth_pb.AuthService/GetUserJwt")
    );
    assert_eq!(
        requests[1].url,
        "https://chat.example/exa.api_server_pb.ApiServerService/GetChatMessage"
    );
    assert_eq!(requests[1].body[0], 1);
}

#[test]
fn assistant_and_tool_prompts_have_stable_ids_and_fields() {
    let prompts = build_chat_message_prompts(
        &[
            DevinMessage::Assistant {
                response_id: Some("assistant-1".into()),
                content: vec![widevin::DevinAssistantContentPart::ToolCall {
                    id: "call-1".into(),
                    name: "search".into(),
                    arguments: serde_json::json!({"q":"x"}),
                }],
            },
            DevinMessage::Tool {
                tool_call_id: "call-1".into(),
                content: vec![DevinContentPart::Text {
                    text: "result".into(),
                }],
                is_error: true,
            },
        ],
        "cascade-1",
    );
    assert_eq!(prompts[0].message_id, "assistant-1");
    assert_eq!(prompts[0].tool_calls[0].arguments_json, "{\"q\":\"x\"}");
    assert_eq!(prompts[1].tool_call_id, "call-1");
    assert!(prompts[1].tool_result_is_error);
}
