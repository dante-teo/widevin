use crate::{
    DevinError, FetchLike, FetchRequest, OpenBrowser,
    constants::{
        DEVIN_APP_BASE_URL, DEVIN_AUTH_BASE_URL, DEVIN_OAUTH_CALLBACK_PATH,
        DEVIN_OAUTH_CALLBACK_PORT, DEVIN_OAUTH_TOKEN_PATH, trim_trailing_slashes,
    },
    fetch::{default_fetch, read_all},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use indexmap::IndexMap;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use url::Url;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PkcePair {
    pub verifier: String,
    pub challenge: String,
}

#[derive(Clone, Debug)]
pub struct BuildDevinAuthUrlOptions {
    pub app_base_url: String,
    pub redirect_uri: String,
    pub state: String,
    pub challenge: String,
}

impl Default for BuildDevinAuthUrlOptions {
    fn default() -> Self {
        Self {
            app_base_url: DEVIN_APP_BASE_URL.into(),
            redirect_uri: String::new(),
            state: String::new(),
            challenge: String::new(),
        }
    }
}

#[derive(Clone)]
pub struct ExchangeDevinCliTokenOptions {
    pub code: String,
    pub verifier: String,
    pub fetch: Option<FetchLike>,
    pub auth_base_url: String,
}

impl Default for ExchangeDevinCliTokenOptions {
    fn default() -> Self {
        Self {
            code: String::new(),
            verifier: String::new(),
            fetch: None,
            auth_base_url: DEVIN_AUTH_BASE_URL.into(),
        }
    }
}

#[derive(Clone)]
pub struct DevinLoginOptions {
    pub fetch: Option<FetchLike>,
    pub open_browser: Option<OpenBrowser>,
    pub app_base_url: String,
    pub auth_base_url: String,
}

impl Default for DevinLoginOptions {
    fn default() -> Self {
        Self {
            fetch: None,
            open_browser: None,
            app_base_url: DEVIN_APP_BASE_URL.into(),
            auth_base_url: DEVIN_AUTH_BASE_URL.into(),
        }
    }
}

pub fn create_pkce_pair() -> PkcePair {
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let bytes = [first.as_bytes().as_slice(), second.as_bytes().as_slice()].concat();
    let verifier = URL_SAFE_NO_PAD.encode(bytes);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    PkcePair {
        verifier,
        challenge,
    }
}

/// Builds the Devin CLI continuation URL.
///
/// # Errors
///
/// Returns [`DevinError::Auth`] when `app_base_url` is not a valid URL.
pub fn build_devin_auth_url(options: &BuildDevinAuthUrlOptions) -> Result<String, DevinError> {
    let mut url = Url::parse(&format!(
        "{}/auth/cli/continue",
        trim_trailing_slashes(&options.app_base_url)
    ))
    .map_err(|error| DevinError::auth(format!("Invalid Devin app URL: {error}")))?;
    url.query_pairs_mut()
        .append_pair("redirect_uri", &options.redirect_uri)
        .append_pair("state", &options.state)
        .append_pair("prompt", "select_account")
        .append_pair("code_challenge", &options.challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(url.into())
}

/// Exchanges a callback code and PKCE verifier for a raw Devin token.
///
/// # Errors
///
/// Returns [`DevinError::Auth`] for transport, HTTP, JSON, or empty-token
/// failures.
pub async fn exchange_devin_cli_token(
    options: ExchangeDevinCliTokenOptions,
) -> Result<String, DevinError> {
    let fetch = options.fetch.unwrap_or_else(default_fetch);
    let body = serde_json::to_vec(&serde_json::json!({
        "code": options.code,
        "code_verifier": options.verifier,
    }))
    .map_err(|error| DevinError::auth(format!("Failed to encode Devin token request: {error}")))?;
    let response = fetch(FetchRequest {
        url: format!(
            "{}{DEVIN_OAUTH_TOKEN_PATH}",
            trim_trailing_slashes(&options.auth_base_url)
        ),
        method: "POST".into(),
        headers: IndexMap::from([
            ("Accept".into(), "application/json".into()),
            ("Content-Type".into(), "application/json".into()),
        ]),
        body,
    })
    .await
    .map_err(|error| DevinError::auth(format!("Devin CLI token exchange failed: {error}")))?;
    let status = response.status;
    let payload = read_all(response)
        .await
        .map_err(|error| DevinError::auth(format!("Devin CLI token exchange failed: {error}")))?;
    if !(200..300).contains(&status) {
        return Err(DevinError::auth(
            format!(
                "Devin CLI token exchange failed: {status} {}",
                String::from_utf8_lossy(&payload)
            )
            .trim()
            .to_owned(),
        ));
    }
    let parsed = serde_json::from_slice::<Value>(&payload).map_err(|error| {
        DevinError::auth(format!(
            "Devin CLI token exchange returned invalid JSON: {error}"
        ))
    })?;
    parsed
        .get("token")
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| DevinError::auth("Devin CLI token exchange returned an empty token"))
}

/// Validates the callback path, state, and authorization code.
///
/// # Errors
///
/// Returns [`DevinError::Auth`] when any callback parameter is invalid.
pub fn validate_devin_oauth_callback(url: &Url, state: &str) -> Result<String, DevinError> {
    if url.path() != DEVIN_OAUTH_CALLBACK_PATH {
        return Err(DevinError::auth("Unexpected Devin OAuth callback path"));
    }
    let parameter = |name: &str| {
        url.query_pairs()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.into_owned())
    };
    if parameter("state").as_deref() != Some(state) {
        return Err(DevinError::auth("Invalid Devin OAuth callback state"));
    }
    parameter("code")
        .filter(|code| !code.is_empty())
        .ok_or_else(|| DevinError::auth("Missing Devin OAuth callback code"))
}

/// Runs the loopback OAuth flow and returns a raw Devin token.
///
/// # Errors
///
/// Returns [`DevinError::Auth`] when the callback server, browser hook,
/// callback validation, or token exchange fails.
pub async fn login_devin(options: DevinLoginOptions) -> Result<String, DevinError> {
    let pkce = create_pkce_pair();
    let state = Uuid::new_v4().to_string();
    let listener = TcpListener::bind(("127.0.0.1", DEVIN_OAUTH_CALLBACK_PORT))
        .await
        .map_err(|error| {
            DevinError::auth(format!(
                "Failed to start Devin OAuth callback server: {error}"
            ))
        })?;
    let address = listener.local_addr().map_err(|error| {
        DevinError::auth(format!(
            "Failed to inspect Devin OAuth callback server: {error}"
        ))
    })?;
    let redirect_uri = format!(
        "http://127.0.0.1:{}{DEVIN_OAUTH_CALLBACK_PATH}",
        address.port()
    );
    let auth_url = build_devin_auth_url(&BuildDevinAuthUrlOptions {
        app_base_url: options.app_base_url,
        redirect_uri,
        state: state.clone(),
        challenge: pkce.challenge,
    })?;
    if let Some(open_browser) = options.open_browser {
        open_browser(auth_url).await?;
    }
    let (mut stream, _) = listener.accept().await.map_err(|error| {
        DevinError::auth(format!("Failed to accept Devin OAuth callback: {error}"))
    })?;
    let callback = read_callback(&mut stream, &state).await;
    write_callback_response(&mut stream, callback.as_ref().err()).await;
    let code = callback?;
    exchange_devin_cli_token(ExchangeDevinCliTokenOptions {
        code,
        verifier: pkce.verifier,
        fetch: options.fetch,
        auth_base_url: options.auth_base_url,
    })
    .await
}

async fn read_callback(stream: &mut TcpStream, state: &str) -> Result<String, DevinError> {
    const MAX_REQUEST_HEAD_BYTES: usize = 16 * 1024;

    let mut buffer = Vec::new();
    loop {
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if buffer.len() == MAX_REQUEST_HEAD_BYTES {
            return Err(DevinError::auth(
                "Devin OAuth callback request headers are too large",
            ));
        }
        let mut chunk = [0_u8; 1024];
        let remaining = MAX_REQUEST_HEAD_BYTES - buffer.len();
        let read_capacity = remaining.min(chunk.len());
        let length = stream
            .read(&mut chunk[..read_capacity])
            .await
            .map_err(|error| {
                DevinError::auth(format!("Failed to read Devin OAuth callback: {error}"))
            })?;
        if length == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..length]);
    }
    let request = String::from_utf8_lossy(&buffer);
    let target = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .ok_or_else(|| DevinError::auth("Invalid Devin OAuth callback request"))?;
    let url = Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(|error| DevinError::auth(format!("Invalid Devin OAuth callback URL: {error}")))?;
    validate_devin_oauth_callback(&url, state)
}

async fn write_callback_response(stream: &mut TcpStream, error: Option<&DevinError>) {
    let (status, body) = error.map_or(
        (
            "200 OK",
            "Devin authentication complete. You can close this window.".to_owned(),
        ),
        |error| ("400 Bad Request", error.to_string()),
    );
    let response = format!(
        "HTTP/1.1 {status}\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}
