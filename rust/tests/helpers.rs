#![allow(dead_code, clippy::missing_errors_doc, clippy::missing_panics_doc)]

use futures_util::StreamExt;
use std::sync::Arc;
use widevin::{DevinError, FetchLike, FetchRequest, FetchResponse, fetch_response_from_chunks};

pub fn fetch_sequence(
    responses: Vec<Result<FetchResponse, DevinError>>,
) -> (FetchLike, Arc<std::sync::Mutex<Vec<FetchRequest>>>) {
    let responses = Arc::new(std::sync::Mutex::new(responses.into_iter()));
    let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let captured = Arc::clone(&requests);
    let fetch = Arc::new(move |request: FetchRequest| {
        captured.lock().expect("request lock").push(request);
        let response = responses
            .lock()
            .expect("response lock")
            .next()
            .expect("unexpected request");
        Box::pin(async move { response }) as _
    });
    (fetch, requests)
}

pub fn chunked_response(chunks: Vec<Vec<u8>>) -> FetchResponse {
    fetch_response_from_chunks(200, "OK", chunks.into_iter().map(Ok))
}

pub async fn collect_events(
    stream: widevin::DevinEventStream,
) -> Result<Vec<widevin::DevinStreamEvent>, DevinError> {
    stream.collect::<Vec<_>>().await.into_iter().collect()
}
