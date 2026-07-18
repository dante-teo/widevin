use crate::{
    DevinError, DevinInput, DevinModel, FetchLike, FetchRequest,
    connect::decode_proto_with_gzip_fallback,
    constants::{
        DEVIN_DEFAULT_BASE_URL, DEVIN_EXTENSION_VERSION, DEVIN_IDE_VERSION, DEVIN_MODELS_PATH,
        trim_trailing_slashes,
    },
    fetch::{default_fetch, read_all},
    normalize_devin_session_token,
    proto::{ClientModelConfig, GetCliModelConfigsRequest, GetCliModelConfigsResponse, Metadata},
};
use indexmap::IndexMap;
use prost::Message;

const DEFAULT_CONTEXT_WINDOW: i32 = 200_000;
const DEFAULT_MAX_TOKENS: i32 = 64_000;

#[derive(Clone)]
pub struct ListDevinModelsOptions {
    pub token: Option<String>,
    pub base_url: String,
    pub fetch: Option<FetchLike>,
}

impl Default for ListDevinModelsOptions {
    fn default() -> Self {
        Self {
            token: None,
            base_url: DEVIN_DEFAULT_BASE_URL.into(),
            fetch: None,
        }
    }
}

/// Discovers and normalizes the enabled Devin models.
///
/// # Errors
///
/// Returns [`DevinError::Api`] for transport/HTTP failures and
/// [`DevinError::Protocol`] for malformed protobuf responses.
pub async fn list_devin_models(
    options: ListDevinModelsOptions,
) -> Result<Vec<DevinModel>, DevinError> {
    let base_url = trim_trailing_slashes(&options.base_url);
    let model_base_url = options.base_url.clone();
    let request = GetCliModelConfigsRequest {
        metadata: Some(metadata(
            &normalize_devin_session_token(options.token.as_deref()),
            "",
        )),
    };
    let fetch = options.fetch.unwrap_or_else(default_fetch);
    let response = fetch(FetchRequest {
        url: format!("{base_url}{DEVIN_MODELS_PATH}"),
        method: "POST".into(),
        headers: IndexMap::from([
            ("content-type".into(), "application/proto".into()),
            ("connect-protocol-version".into(), "1".into()),
            ("accept".into(), "*/*".into()),
        ]),
        body: request.encode_to_vec(),
    })
    .await?;
    let status = response.status;
    let status_text = response.status_text.clone();
    let payload = read_all(response).await?;
    if !(200..300).contains(&status) {
        return Err(DevinError::api(
            format!("Devin model discovery failed: {status} {status_text}"),
            status,
            Some(String::from_utf8_lossy(&payload).into_owned()),
        ));
    }
    let decoded = decode_proto_with_gzip_fallback::<GetCliModelConfigsResponse>(&payload)?;
    Ok(normalize_models(
        &decoded.client_model_configs,
        &model_base_url,
    ))
}

pub fn normalize_models(configs: &[ClientModelConfig], base_url: &str) -> Vec<DevinModel> {
    let mut models = configs
        .iter()
        .filter(|config| !config.disabled)
        .filter_map(|config| {
            let model_uid = config.model_uid.trim();
            (!model_uid.is_empty()).then(|| {
                let context_window = if config.max_tokens > 0 {
                    config.max_tokens
                } else {
                    DEFAULT_CONTEXT_WINDOW
                };
                (
                    model_uid.to_owned(),
                    DevinModel {
                        id: model_uid.to_owned(),
                        name: {
                            let label = config.label.trim();
                            if label.is_empty() {
                                model_uid.to_owned()
                            } else {
                                label.to_owned()
                            }
                        },
                        provider: "devin",
                        base_url: base_url.to_owned(),
                        input: if config.supports_images {
                            vec![DevinInput::Text, DevinInput::Image]
                        } else {
                            vec![DevinInput::Text]
                        },
                        supports_tools: true,
                        reasoning: supports_thinking(config),
                        context_window,
                        max_tokens: if config.max_tokens > 0 {
                            config.max_tokens.min(DEFAULT_MAX_TOKENS)
                        } else {
                            DEFAULT_MAX_TOKENS
                        },
                    },
                )
            })
        })
        .fold(
            IndexMap::<String, DevinModel>::new(),
            |mut by_id, (id, model)| {
                by_id.insert(id, model);
                by_id
            },
        )
        .into_values()
        .collect::<Vec<_>>();
    models.sort_by(|left, right| left.id.cmp(&right.id));
    models
}

pub fn metadata(api_key: &str, user_jwt: &str) -> Metadata {
    Metadata {
        api_key: api_key.into(),
        user_jwt: user_jwt.into(),
        ide_name: "windsurf".into(),
        ide_version: DEVIN_IDE_VERSION.into(),
        extension_name: "windsurf".into(),
        extension_version: DEVIN_EXTENSION_VERSION.into(),
        locale: "en".into(),
    }
}

fn supports_thinking(config: &ClientModelConfig) -> bool {
    let label = config.label.to_lowercase();
    if contains_ascii_word_phrase(&label, "no thinking") {
        return false;
    }
    config
        .model_info
        .as_ref()
        .and_then(|info| info.model_features.as_ref())
        .is_some_and(|features| features.supports_thinking)
        || [
            "think",
            "thinking",
            "minimal",
            "high",
            "medium",
            "low",
            "xhigh",
            "max",
            "reasoning",
        ]
        .iter()
        .any(|term| label.contains(term))
}

fn contains_ascii_word_phrase(value: &str, phrase: &str) -> bool {
    value.match_indices(phrase).any(|(start, matched)| {
        let before = value[..start].chars().next_back();
        let after = value[start + matched.len()..].chars().next();
        before.is_none_or(|character| !is_ascii_word(character))
            && after.is_none_or(|character| !is_ascii_word(character))
    })
}

fn is_ascii_word(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}
