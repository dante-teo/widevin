use base64::{Engine, engine::general_purpose::STANDARD};
use prost::Message;
use widevin::proto::{GetCliModelConfigsRequest, GetCliModelConfigsResponse};

const TYPESCRIPT_RESPONSE: &str =
    include_str!("../fixtures/protobuf/typescript-models-response.base64");
const RUST_REQUEST: &str = include_str!("../fixtures/protobuf/rust-models-request.base64");

#[test]
fn typescript_response_decodes_in_rust() {
    let bytes = STANDARD
        .decode(TYPESCRIPT_RESPONSE.trim())
        .expect("base64 fixture");
    let response =
        GetCliModelConfigsResponse::decode(bytes.as_slice()).expect("TypeScript protobuf");
    let model = &response.client_model_configs[0];
    assert_eq!(model.model_uid, "fixture-model");
    assert_eq!(model.label, "Fixture Thinking");
    assert!(model.supports_images);
    assert_eq!(model.max_tokens, 4_096);
}

#[test]
fn rust_request_fixture_stays_equal_to_prost_encoding() {
    let request = GetCliModelConfigsRequest {
        metadata: Some(widevin::metadata("devin-session-token$fixture", "")),
    };
    assert_eq!(
        STANDARD.encode(request.encode_to_vec()),
        RUST_REQUEST.trim()
    );
}
