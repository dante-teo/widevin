mod helpers;

use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use widevin::{
    BuildDevinAuthUrlOptions, DevinError, DevinLoginOptions, ExchangeDevinCliTokenOptions,
    build_devin_auth_url, create_pkce_pair, exchange_devin_cli_token, fetch_response_from_bytes,
    login_devin, validate_devin_oauth_callback,
};

#[test]
fn pkce_url_and_callback_validation_match_typescript() {
    let pair = create_pkce_pair();
    let challenge = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        Sha256::digest(pair.verifier.as_bytes()),
    );
    assert_eq!(pair.challenge, challenge);

    let url = build_devin_auth_url(&BuildDevinAuthUrlOptions {
        app_base_url: "https://app.example".into(),
        redirect_uri: "http://127.0.0.1:59653/callback".into(),
        state: "state-1".into(),
        challenge: "challenge-1".into(),
    })
    .expect("auth URL");
    let parsed = url::Url::parse(&url).expect("parse URL");
    assert_eq!(parsed.path(), "/auth/cli/continue");
    assert_eq!(
        parsed
            .query_pairs()
            .find(|(key, _)| key == "code_challenge")
            .map(|(_, value)| value.into_owned())
            .as_deref(),
        Some("challenge-1")
    );

    assert_eq!(
        validate_devin_oauth_callback(
            &url::Url::parse("http://127.0.0.1:59653/callback?state=state-1&code=code-1")
                .expect("callback"),
            "state-1"
        )
        .expect("callback code"),
        "code-1"
    );
    assert!(matches!(
        validate_devin_oauth_callback(
            &url::Url::parse("http://127.0.0.1:59653/callback?state=wrong&code=code-1")
                .expect("callback"),
            "state-1"
        ),
        Err(DevinError::Auth { .. })
    ));
}

#[tokio::test]
async fn login_closes_callback_listener_after_success_and_browser_failure() {
    let (fetch, _) = helpers::fetch_sequence(vec![Ok(fetch_response_from_bytes(
        200,
        "OK",
        br#"{"token":"oauth-token"}"#.to_vec(),
    ))]);
    let browser = Arc::new(|auth_url: String| {
        Box::pin(async move {
            tokio::spawn(async move {
                let auth_url = url::Url::parse(&auth_url).expect("auth URL");
                let parameter = |name: &str| {
                    auth_url
                        .query_pairs()
                        .find(|(key, _)| key == name)
                        .map(|(_, value)| value.into_owned())
                };
                let redirect =
                    url::Url::parse(&parameter("redirect_uri").expect("redirect parameter"))
                        .expect("redirect URL");
                let state = parameter("state").expect("state parameter");
                let mut stream = TcpStream::connect((
                    redirect.host_str().unwrap_or("127.0.0.1"),
                    redirect.port().unwrap_or(59_653),
                ))
                .await
                .expect("connect callback");
                let request = format!(
                    "GET /callback?state={state}&code=code-1 HTTP/1.1\r\nhost: 127.0.0.1\r\n\r\n"
                );
                stream
                    .write_all(&request.as_bytes()[..4])
                    .await
                    .expect("write first callback fragment");
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                stream
                    .write_all(&request.as_bytes()[4..])
                    .await
                    .expect("write remaining callback fragment");
                let mut response = Vec::new();
                stream
                    .read_to_end(&mut response)
                    .await
                    .expect("read callback response");
                assert!(String::from_utf8_lossy(&response).contains("200 OK"));
            });
            Ok(())
        }) as widevin::OpenBrowserFuture
    });
    let token = login_devin(DevinLoginOptions {
        fetch: Some(fetch),
        open_browser: Some(browser),
        ..Default::default()
    })
    .await
    .expect("login");
    assert_eq!(token, "oauth-token");
    let listener = TcpListener::bind(("127.0.0.1", 59_653))
        .await
        .expect("callback listener was released");
    drop(listener);

    let failing_browser = Arc::new(|_: String| {
        Box::pin(async { Err(DevinError::auth("browser failed")) }) as widevin::OpenBrowserFuture
    });
    assert!(matches!(
        login_devin(DevinLoginOptions {
            open_browser: Some(failing_browser),
            ..Default::default()
        })
        .await,
        Err(DevinError::Auth { .. })
    ));
    TcpListener::bind(("127.0.0.1", 59_653))
        .await
        .expect("callback listener was released after browser failure");
}

#[tokio::test]
async fn token_exchange_sends_json_and_reports_transport_and_payload_errors() {
    let (fetch, requests) = helpers::fetch_sequence(vec![Ok(fetch_response_from_bytes(
        200,
        "OK",
        br#"{"token":"raw-token"}"#.to_vec(),
    ))]);
    let token = exchange_devin_cli_token(ExchangeDevinCliTokenOptions {
        code: "code-1".into(),
        verifier: "verifier-1".into(),
        auth_base_url: "https://api.example".into(),
        fetch: Some(fetch),
    })
    .await
    .expect("token");
    assert_eq!(token, "raw-token");
    {
        let request = requests.lock().expect("requests");
        assert_eq!(request[0].url, "https://api.example/auth/cli/token");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&request[0].body).expect("json"),
            serde_json::json!({"code":"code-1","code_verifier":"verifier-1"})
        );
    }

    let fetch = Arc::new(|_: widevin::FetchRequest| {
        Box::pin(async { Err(DevinError::api_transport("offline")) }) as widevin::FetchFuture
    });
    assert!(matches!(
        exchange_devin_cli_token(ExchangeDevinCliTokenOptions {
            code: "code".into(),
            verifier: "verifier".into(),
            fetch: Some(fetch),
            ..Default::default()
        })
        .await,
        Err(DevinError::Auth { .. })
    ));

    let (fetch, _) = helpers::fetch_sequence(vec![Ok(fetch_response_from_bytes(
        200,
        "OK",
        br#"{"token":""}"#.to_vec(),
    ))]);
    assert!(matches!(
        exchange_devin_cli_token(ExchangeDevinCliTokenOptions {
            code: "code".into(),
            verifier: "verifier".into(),
            fetch: Some(fetch),
            ..Default::default()
        })
        .await,
        Err(DevinError::Auth { .. })
    ));
}
