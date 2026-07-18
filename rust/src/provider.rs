use crate::{
    DevinChatRequest, DevinError, DevinEventStream, DevinLoginOptions, DevinModel,
    DevinProviderOptions, ListDevinModelsOptions, StreamDevinChatOptions, TokenStoreRef,
    create_memory_token_store, fetch::default_fetch, list_devin_models, login_devin,
    stream_devin_chat,
};

#[derive(Clone)]
pub struct DevinProvider {
    token_store: TokenStoreRef,
    options: DevinProviderOptions,
}

pub fn create_devin_provider(options: DevinProviderOptions) -> DevinProvider {
    DevinProvider {
        token_store: options
            .token_store
            .clone()
            .unwrap_or_else(|| create_memory_token_store(None)),
        options,
    }
}

impl DevinProvider {
    /// Logs in and stores the resulting raw token.
    ///
    /// # Errors
    ///
    /// Returns an auth error from login or a protocol error from the store.
    pub async fn login(&self) -> Result<String, DevinError> {
        let token = login_devin(DevinLoginOptions {
            fetch: self.options.fetch.clone(),
            open_browser: self.options.open_browser.clone(),
            app_base_url: self.options.app_base_url.clone(),
            auth_base_url: self.options.auth_base_url.clone(),
        })
        .await?;
        self.token_store.set(token.clone()).await?;
        Ok(token)
    }

    /// Replaces the stored raw token.
    ///
    /// # Errors
    ///
    /// Returns [`DevinError::Protocol`] when the token store fails.
    pub async fn set_token(&self, token: impl Into<String>) -> Result<(), DevinError> {
        self.token_store.set(token.into()).await
    }

    /// Clears the stored token.
    ///
    /// # Errors
    ///
    /// Returns [`DevinError::Protocol`] when the token store fails.
    pub async fn clear_token(&self) -> Result<(), DevinError> {
        self.token_store.clear().await
    }

    /// Discovers models using the current stored token.
    ///
    /// # Errors
    ///
    /// Returns a store, transport, HTTP, or protobuf error.
    pub async fn list_models(&self) -> Result<Vec<DevinModel>, DevinError> {
        let token = self.token_store.get().await?;
        list_devin_models(ListDevinModelsOptions {
            token,
            base_url: self.options.base_url.clone(),
            fetch: Some(self.options.fetch.clone().unwrap_or_else(default_fetch)),
        })
        .await
    }

    pub fn stream_chat(&self, request: DevinChatRequest) -> DevinEventStream {
        let store = self.token_store.clone();
        let base_url = self.options.base_url.clone();
        let fetch = self.options.fetch.clone().unwrap_or_else(default_fetch);
        let uuid = self.options.uuid.clone();
        Box::pin(async_stream::try_stream! {
            let token = store.get().await?;
            let stream = stream_devin_chat(StreamDevinChatOptions::from_request(
                request,
                token,
                base_url,
                fetch,
                uuid,
            ));
            futures_util::pin_mut!(stream);
            while let Some(event) = futures_util::StreamExt::next(&mut stream).await {
                yield event?;
            }
        })
    }
}
