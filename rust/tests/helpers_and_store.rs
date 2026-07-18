#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use widevin::{
    create_file_token_store, create_memory_token_store, deterministic_uuid,
    normalize_devin_session_token, parse_possibly_complete_json_throttled, parse_streaming_json,
};

#[test]
fn json_uuid_and_token_helpers_match_typescript() {
    assert_eq!(parse_streaming_json(None), serde_json::json!({}));
    assert_eq!(parse_streaming_json(Some("{")), serde_json::json!({}));
    assert_eq!(parse_streaming_json(Some("null")), serde_json::Value::Null);
    assert_eq!(
        deterministic_uuid(concat!("cascade-1\0", "1\0tool\0call-1")),
        "6225ae28-44b7-53fa-aad2-79b48041b84a"
    );
    assert_eq!(
        normalize_devin_session_token(Some("raw")),
        "devin-session-token$raw"
    );
    assert_eq!(
        normalize_devin_session_token(Some("devin-session-token$raw")),
        "devin-session-token$raw"
    );
    assert_eq!(normalize_devin_session_token(Some("")), "");
    assert_eq!(normalize_devin_session_token(None), "");

    let first = parse_possibly_complete_json_throttled("{\"q\":", 0)
        .expect("first non-empty value is parsed");
    assert_eq!(first.attempted_length, 5);
    assert!(first.value.is_none());
    assert!(
        parse_possibly_complete_json_throttled(
            &format!("{{\"q\":{}", " ".repeat(255)),
            first.attempted_length
        )
        .is_none()
    );
    assert_eq!(
        parse_possibly_complete_json_throttled("😀", 0)
            .expect("unicode parse attempt")
            .attempted_length,
        2
    );
}

#[tokio::test]
async fn memory_and_file_stores_round_trip_and_use_private_permissions() {
    let memory = create_memory_token_store(Some("initial".into()));
    assert_eq!(
        memory.get().await.expect("memory get").as_deref(),
        Some("initial")
    );
    memory.set("next".into()).await.expect("memory set");
    assert_eq!(
        memory.get().await.expect("memory get").as_deref(),
        Some("next")
    );
    memory.clear().await.expect("memory clear");
    assert_eq!(memory.get().await.expect("memory get"), None);

    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("nested/token");
    let file = create_file_token_store(&path);
    file.set("secret".into()).await.expect("file set");
    assert_eq!(
        file.get().await.expect("file get").as_deref(),
        Some("secret")
    );
    #[cfg(unix)]
    assert_eq!(
        std::fs::metadata(&path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    file.clear().await.expect("file clear");
    assert_eq!(file.get().await.expect("file get"), None);
}
