mod helpers;

use prost::Message;
use widevin::{
    DevinError, DevinMessage, DevinProviderOptions, DevinStopReason, DevinStreamEvent,
    create_devin_provider, create_memory_token_store, create_token_store,
    fetch_response_from_bytes, proto,
};

#[tokio::test]
async fn provider_wires_store_fetch_base_url_and_uuid() {
    let models = proto::GetCliModelConfigsResponse {
        client_model_configs: vec![proto::ClientModelConfig {
            model_uid: "model-a".into(),
            label: "Model A".into(),
            ..Default::default()
        }],
    };
    let auth = proto::GetUserJwtResponse {
        user_jwt: "jwt".into(),
        custom_api_server_url: String::new(),
    };
    let done = proto::GetChatMessageResponse {
        stop_reason: proto::StopReason::StopPattern as i32,
        ..Default::default()
    };
    let (fetch, requests) = helpers::fetch_sequence(vec![
        Ok(fetch_response_from_bytes(200, "OK", models.encode_to_vec())),
        Ok(fetch_response_from_bytes(200, "OK", auth.encode_to_vec())),
        Ok(helpers::chunked_response(vec![
            widevin::encode_connect_frame(&done.encode_to_vec(), false, false).expect("frame"),
        ])),
    ]);
    let store = create_memory_token_store(Some("raw".into()));
    let provider = create_devin_provider(DevinProviderOptions {
        token_store: Some(store),
        fetch: Some(fetch),
        base_url: "https://server.example".into(),
        uuid: Some(std::sync::Arc::new(|| "stable-id".into())),
        ..Default::default()
    });
    assert_eq!(
        provider.list_models().await.expect("models")[0].id,
        "model-a"
    );
    let events = helpers::collect_events(provider.stream_chat(widevin::DevinChatRequest {
        model: "model-a".into(),
        messages: Vec::<DevinMessage>::new(),
        ..Default::default()
    }))
    .await
    .expect("chat");
    assert_eq!(
        events.last(),
        Some(&DevinStreamEvent::Done {
            reason: DevinStopReason::Stop
        })
    );
    assert_eq!(requests.lock().expect("requests").len(), 3);
}

#[tokio::test]
async fn provider_propagates_custom_store_failures() {
    let store = create_token_store(
        || Box::pin(async { Err(DevinError::protocol("store unavailable")) }),
        |_| Box::pin(async { Ok(()) }),
        || Box::pin(async { Ok(()) }),
    );
    let provider = create_devin_provider(DevinProviderOptions {
        token_store: Some(store),
        ..Default::default()
    });

    assert!(matches!(
        provider.list_models().await,
        Err(DevinError::Protocol { .. })
    ));
    let events =
        helpers::collect_events(provider.stream_chat(widevin::DevinChatRequest::default())).await;
    assert!(matches!(events, Err(DevinError::Protocol { .. })));
}
