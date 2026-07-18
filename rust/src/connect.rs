use crate::DevinError;
use async_stream::stream;
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use futures_core::Stream;
use futures_util::StreamExt;
use serde_json::Value;
use std::{
    io::{Read, Write},
    pin::Pin,
};

pub const CONNECT_COMPRESSED_FLAG: u8 = 0x01;
pub const CONNECT_END_STREAM_FLAG: u8 = 0x02;
pub const MAX_CONNECT_FRAME_PAYLOAD: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectFrame {
    pub flags: u8,
    pub payload: Vec<u8>,
    pub end_stream: bool,
}

/// Encodes one Connect protocol frame.
///
/// # Errors
///
/// Returns [`DevinError::Protocol`] when compression or length conversion
/// fails.
pub fn encode_connect_frame(
    payload: &[u8],
    compress: bool,
    end_stream: bool,
) -> Result<Vec<u8>, DevinError> {
    let body = if compress {
        gzip(payload)?
    } else {
        payload.to_vec()
    };
    let length = u32::try_from(body.len())
        .map_err(|_| DevinError::protocol("Devin Connect frame payload is too large"))?;
    let flags = (u8::from(compress) * CONNECT_COMPRESSED_FLAG)
        | (u8::from(end_stream) * CONNECT_END_STREAM_FLAG);
    Ok([vec![flags], length.to_be_bytes().to_vec(), body].concat())
}

pub fn decode_connect_frames<S>(
    chunks: S,
) -> Pin<Box<dyn Stream<Item = Result<ConnectFrame, DevinError>> + Send>>
where
    S: Stream<Item = Result<Vec<u8>, DevinError>> + Send + 'static,
{
    Box::pin(stream! {
        futures_util::pin_mut!(chunks);
        let mut pending = Vec::new();
        while let Some(chunk) = chunks.next().await {
            match chunk {
                Err(error) => {
                    yield Err(error);
                    return;
                }
                Ok(chunk) => pending.extend(chunk),
            }
            loop {
                if pending.len() < 5 {
                    break;
                }
                let flags = pending[0];
                let length = u32::from_be_bytes([
                    pending[1], pending[2], pending[3], pending[4],
                ]) as usize;
                if length > MAX_CONNECT_FRAME_PAYLOAD {
                    yield Err(DevinError::protocol(format!(
                        "Devin Connect frame length {length} exceeds {MAX_CONNECT_FRAME_PAYLOAD}-byte cap"
                    )));
                    return;
                }
                if pending.len() < 5 + length {
                    break;
                }
                let raw = pending[5..5 + length].to_vec();
                pending.drain(..5 + length);
                let payload = if flags & CONNECT_COMPRESSED_FLAG != 0 {
                    match gunzip(&raw) {
                        Ok(payload) => payload,
                        Err(error) => {
                            yield Err(error);
                            return;
                        }
                    }
                } else {
                    raw
                };
                yield Ok(ConnectFrame {
                    flags,
                    payload,
                    end_stream: flags & CONNECT_END_STREAM_FLAG != 0,
                });
            }
        }
        if !pending.is_empty() {
            yield Err(DevinError::protocol(
                "Devin Connect stream ended with a partial frame",
            ));
        }
    })
}

pub fn read_connect_trailer_error(payload: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(payload);
    let parsed = serde_json::from_str::<Value>(text.trim()).ok()?;
    let error = parsed.get("error")?;
    let code = error
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    (!code.is_empty() || !message.is_empty()).then(|| {
        format!(
            "Devin stream error{}: {message}",
            if code.is_empty() {
                String::new()
            } else {
                format!(" {code}")
            }
        )
    })
}

pub(crate) fn decode_proto_with_gzip_fallback<M>(payload: &[u8]) -> Result<M, DevinError>
where
    M: prost::Message + Default,
{
    M::decode(payload).or_else(|original| {
        gunzip(payload)
            .and_then(|decoded| {
                M::decode(decoded.as_slice()).map_err(|error| {
                    DevinError::protocol(format!("invalid Devin protobuf response: {error}"))
                })
            })
            .map_err(|_| {
                DevinError::protocol(format!("invalid Devin protobuf response: {original}"))
            })
    })
}

fn gzip(payload: &[u8]) -> Result<Vec<u8>, DevinError> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(payload)
        .and_then(|()| encoder.finish())
        .map_err(|error| DevinError::protocol(format!("failed to gzip Connect frame: {error}")))
}

fn gunzip(payload: &[u8]) -> Result<Vec<u8>, DevinError> {
    let mut decoder = GzDecoder::new(payload);
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .map(|_| output)
        .map_err(|error| DevinError::protocol(format!("failed to gunzip Connect frame: {error}")))
}
