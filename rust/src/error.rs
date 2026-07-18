/// The three error categories exposed by the TypeScript package.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DevinError {
    /// OAuth, callback, or token-exchange failure.
    #[error("{message}")]
    Auth { message: String },
    /// HTTP or HTTP-transport failure. Transport failures use status `0`.
    #[error("{message}")]
    Api {
        message: String,
        status: u16,
        body: Option<String>,
    },
    /// Protobuf, Connect framing, token-store, or other wire-protocol failure.
    #[error("{message}")]
    Protocol { message: String },
}

impl DevinError {
    pub fn auth(message: impl Into<String>) -> Self {
        Self::Auth {
            message: message.into(),
        }
    }

    pub fn api(message: impl Into<String>, status: u16, body: Option<String>) -> Self {
        Self::Api {
            message: message.into(),
            status,
            body,
        }
    }

    pub fn api_transport(message: impl Into<String>) -> Self {
        let detail = message.into();
        Self::api(format!("Devin transport failed: {detail}"), 0, None)
    }

    pub fn protocol(message: impl Into<String>) -> Self {
        Self::Protocol {
            message: message.into(),
        }
    }
}
