use crate::DevinError;
use futures_core::Stream;
use futures_util::StreamExt;
use indexmap::IndexMap;
use std::{future::Future, pin::Pin, sync::Arc};

pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Vec<u8>, DevinError>> + Send + 'static>>;
pub type FetchFuture =
    Pin<Box<dyn Future<Output = Result<FetchResponse, DevinError>> + Send + 'static>>;
pub type FetchLike = Arc<dyn Fn(FetchRequest) -> FetchFuture + Send + Sync + 'static>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FetchRequest {
    pub url: String,
    pub method: String,
    pub headers: IndexMap<String, String>,
    pub body: Vec<u8>,
}

pub struct FetchResponse {
    pub status: u16,
    pub status_text: String,
    pub body: Option<ByteStream>,
}

impl std::fmt::Debug for FetchResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FetchResponse")
            .field("status", &self.status)
            .field("status_text", &self.status_text)
            .field("body", &"<byte stream>")
            .finish()
    }
}

pub fn fetch_response_from_bytes(
    status: u16,
    status_text: impl Into<String>,
    body: Vec<u8>,
) -> FetchResponse {
    fetch_response_from_chunks(status, status_text, [Ok(body)])
}

pub fn fetch_response_from_chunks<I>(
    status: u16,
    status_text: impl Into<String>,
    chunks: I,
) -> FetchResponse
where
    I: IntoIterator<Item = Result<Vec<u8>, DevinError>>,
    I::IntoIter: Send + 'static,
{
    FetchResponse {
        status,
        status_text: status_text.into(),
        body: Some(Box::pin(futures_util::stream::iter(chunks))),
    }
}

pub(crate) fn default_fetch() -> FetchLike {
    let client = reqwest::Client::new();
    Arc::new(move |request: FetchRequest| {
        let client = client.clone();
        Box::pin(async move {
            let method = request.method.parse::<reqwest::Method>().map_err(|error| {
                DevinError::api_transport(format!("invalid HTTP method: {error}"))
            })?;
            let builder = request
                .headers
                .iter()
                .fold(
                    client.request(method, &request.url),
                    |builder, (name, value)| builder.header(name, value),
                )
                .body(request.body);
            let response = builder
                .send()
                .await
                .map_err(|error| DevinError::api_transport(error.to_string()))?;
            let status = response.status();
            let status_text = status.canonical_reason().unwrap_or_default().to_owned();
            let body = response.bytes_stream().map(|result| {
                result
                    .map(|bytes| bytes.to_vec())
                    .map_err(|error| DevinError::api_transport(error.to_string()))
            });
            Ok(FetchResponse {
                status: status.as_u16(),
                status_text,
                body: Some(Box::pin(body)),
            })
        })
    })
}

pub(crate) async fn read_all(mut response: FetchResponse) -> Result<Vec<u8>, DevinError> {
    let mut output = Vec::new();
    if let Some(mut body) = response.body.take() {
        while let Some(chunk) = body.next().await {
            output.extend(chunk?);
        }
    }
    Ok(output)
}
