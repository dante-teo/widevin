use serde_json::Value;

pub const STREAMING_JSON_PARSE_MIN_GROWTH: usize = 256;

#[derive(Clone, Debug, PartialEq)]
pub struct ThrottledJsonParse {
    pub value: Option<Value>,
    pub attempted_length: usize,
}

pub fn parse_streaming_json(value: Option<&str>) -> Value {
    value
        .filter(|value| !value.is_empty())
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

pub fn parse_possibly_complete_json(value: &str) -> Option<Value> {
    serde_json::from_str(value).ok()
}

pub fn parse_possibly_complete_json_throttled(
    value: &str,
    last_attempted_length: usize,
) -> Option<ThrottledJsonParse> {
    // JavaScript's `String.length` counts UTF-16 code units. Keeping the same
    // unit preserves TypeScript parse-throttling behavior for non-ASCII JSON.
    let attempted_length = value.encode_utf16().count();
    let should_throttle = last_attempted_length > 0
        && attempted_length.saturating_sub(last_attempted_length) < STREAMING_JSON_PARSE_MIN_GROWTH;
    (attempted_length > 0 && !should_throttle).then(|| ThrottledJsonParse {
        value: parse_possibly_complete_json(value),
        attempted_length,
    })
}
