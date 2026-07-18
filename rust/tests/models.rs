mod helpers;

use flate2::{Compression, write::GzEncoder};
use prost::Message;
use std::io::Write;
use widevin::{
    DevinError, ListDevinModelsOptions, fetch_response_from_bytes, list_devin_models,
    proto::{ClientModelConfig, GetCliModelConfigsResponse},
};

#[tokio::test]
async fn models_are_normalized_and_request_has_prefixed_token() {
    let payload = GetCliModelConfigsResponse {
        client_model_configs: vec![
            ClientModelConfig {
                model_uid: " model-a ".into(),
                label: "Model A Thinking".into(),
                supports_images: true,
                max_tokens: 1_000,
                ..Default::default()
            },
            ClientModelConfig {
                model_uid: "disabled".into(),
                label: "Disabled".into(),
                disabled: true,
                ..Default::default()
            },
            ClientModelConfig {
                model_uid: " ".into(),
                label: "Blank".into(),
                ..Default::default()
            },
            ClientModelConfig {
                model_uid: "model-no-thinking".into(),
                label: "Claude (No Thinking)".into(),
                ..Default::default()
            },
        ],
    }
    .encode_to_vec();
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&payload).expect("gzip write");
    let (fetch, requests) = helpers::fetch_sequence(vec![Ok(fetch_response_from_bytes(
        200,
        "OK",
        encoder.finish().expect("gzip finish"),
    ))]);

    let models = list_devin_models(ListDevinModelsOptions {
        token: Some("raw".into()),
        base_url: "https://server.example/".into(),
        fetch: Some(fetch),
    })
    .await
    .expect("models");
    assert_eq!(models.len(), 2);
    assert_eq!(models[0].id, "model-a");
    assert_eq!(models[0].base_url, "https://server.example/");
    assert_eq!(
        models[0].input,
        vec![widevin::DevinInput::Text, widevin::DevinInput::Image]
    );
    assert!(models[0].reasoning);
    assert_eq!(models[1].id, "model-no-thinking");
    assert!(!models[1].reasoning);

    let request = requests.lock().expect("requests");
    assert_eq!(
        request[0].url,
        "https://server.example/exa.api_server_pb.ApiServerService/GetCliModelConfigs"
    );
    let decoded = widevin::proto::GetCliModelConfigsRequest::decode(request[0].body.as_slice())
        .expect("request protobuf");
    assert_eq!(
        decoded.metadata.expect("metadata").api_key,
        "devin-session-token$raw"
    );
}

#[tokio::test]
async fn model_http_and_malformed_payload_errors_are_structured() {
    let (fetch, _) = helpers::fetch_sequence(vec![Ok(fetch_response_from_bytes(
        500,
        "Nope",
        b"nope".to_vec(),
    ))]);
    let error = list_devin_models(ListDevinModelsOptions {
        fetch: Some(fetch),
        ..Default::default()
    })
    .await
    .expect_err("http error");
    assert!(matches!(error, DevinError::Api { status: 500, .. }));

    let (fetch, _) =
        helpers::fetch_sequence(vec![Ok(fetch_response_from_bytes(200, "OK", vec![0xff]))]);
    let error = list_devin_models(ListDevinModelsOptions {
        fetch: Some(fetch),
        ..Default::default()
    })
    .await
    .expect_err("protocol error");
    assert!(matches!(error, DevinError::Protocol { .. }));
}
