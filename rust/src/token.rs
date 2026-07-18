use crate::{DevinError, constants::DEVIN_SESSION_TOKEN_PREFIX};
use std::{
    future::Future,
    path::Path,
    pin::Pin,
    sync::{Arc, Mutex},
};

pub type TokenGetFuture =
    Pin<Box<dyn Future<Output = Result<Option<String>, DevinError>> + Send + 'static>>;
pub type TokenStoreFuture = Pin<Box<dyn Future<Output = Result<(), DevinError>> + Send + 'static>>;
type GetToken = Arc<dyn Fn() -> TokenGetFuture + Send + Sync>;
type SetToken = Arc<dyn Fn(String) -> TokenStoreFuture + Send + Sync>;
type ClearToken = Arc<dyn Fn() -> TokenStoreFuture + Send + Sync>;

pub trait TokenStore: Send + Sync {
    fn get(&self) -> TokenGetFuture;
    fn set(&self, token: String) -> TokenStoreFuture;
    fn clear(&self) -> TokenStoreFuture;
}

pub type TokenStoreRef = Arc<dyn TokenStore>;

struct ClosureTokenStore {
    get: GetToken,
    set: SetToken,
    clear: ClearToken,
}

impl TokenStore for ClosureTokenStore {
    fn get(&self) -> TokenGetFuture {
        (self.get)()
    }

    fn set(&self, token: String) -> TokenStoreFuture {
        (self.set)(token)
    }

    fn clear(&self) -> TokenStoreFuture {
        (self.clear)()
    }
}

pub fn create_token_store(
    get: impl Fn() -> TokenGetFuture + Send + Sync + 'static,
    set: impl Fn(String) -> TokenStoreFuture + Send + Sync + 'static,
    clear: impl Fn() -> TokenStoreFuture + Send + Sync + 'static,
) -> TokenStoreRef {
    Arc::new(ClosureTokenStore {
        get: Arc::new(get),
        set: Arc::new(set),
        clear: Arc::new(clear),
    })
}

pub fn create_memory_token_store(initial_token: Option<String>) -> TokenStoreRef {
    let token = Arc::new(Mutex::new(initial_token));
    let get_token = Arc::clone(&token);
    let set_token = Arc::clone(&token);
    let clear_token = Arc::clone(&token);
    create_token_store(
        move || {
            let result = get_token
                .lock()
                .map(|value| value.clone())
                .map_err(|error| DevinError::protocol(format!("token store lock failed: {error}")));
            Box::pin(async move { result })
        },
        move |next| {
            let result = set_token
                .lock()
                .map(|mut value| *value = Some(next))
                .map_err(|error| DevinError::protocol(format!("token store lock failed: {error}")));
            Box::pin(async move { result })
        },
        move || {
            let result = clear_token
                .lock()
                .map(|mut value| *value = None)
                .map_err(|error| DevinError::protocol(format!("token store lock failed: {error}")));
            Box::pin(async move { result })
        },
    )
}

pub fn create_file_token_store(path: impl AsRef<Path>) -> TokenStoreRef {
    let path = Arc::new(path.as_ref().to_path_buf());
    let get_path = Arc::clone(&path);
    let set_path = Arc::clone(&path);
    create_token_store(
        move || {
            let path = Arc::clone(&get_path);
            Box::pin(async move {
                match tokio::fs::read_to_string(path.as_ref()).await {
                    Ok(value) => {
                        let trimmed = value.trim();
                        Ok((!trimmed.is_empty()).then(|| trimmed.to_owned()))
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                    Err(error) => Err(store_error("read", path.as_ref(), &error)),
                }
            })
        },
        move |token| {
            let path = Arc::clone(&set_path);
            Box::pin(async move {
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent).await.map_err(|error| {
                        store_error("create directory for", path.as_ref(), &error)
                    })?;
                }
                write_token_file(path.as_ref(), token.as_bytes()).await?;
                set_private_permissions(path.as_ref()).await
            })
        },
        move || {
            let path = Arc::clone(&path);
            Box::pin(async move {
                match tokio::fs::remove_file(path.as_ref()).await {
                    Ok(()) => Ok(()),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    Err(error) => Err(store_error("remove", path.as_ref(), &error)),
                }
            })
        },
    )
}

pub fn normalize_devin_session_token(token: Option<&str>) -> String {
    token
        .filter(|token| !token.is_empty())
        .map_or_else(String::new, |token| {
            if token.starts_with(DEVIN_SESSION_TOKEN_PREFIX) {
                token.to_owned()
            } else {
                format!("{DEVIN_SESSION_TOKEN_PREFIX}{token}")
            }
        })
}

fn store_error(action: &str, path: &Path, error: &std::io::Error) -> DevinError {
    DevinError::protocol(format!(
        "failed to {action} token file {}: {error}",
        path.display()
    ))
}

#[cfg(unix)]
async fn write_token_file(path: &Path, token: &[u8]) -> Result<(), DevinError> {
    use tokio::io::AsyncWriteExt;

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .await
        .map_err(|error| store_error("open", path, &error))?;
    file.write_all(token)
        .await
        .map_err(|error| store_error("write", path, &error))
}

#[cfg(not(unix))]
async fn write_token_file(path: &Path, token: &[u8]) -> Result<(), DevinError> {
    tokio::fs::write(path, token)
        .await
        .map_err(|error| store_error("write", path, &error))
}

#[cfg(unix)]
async fn set_private_permissions(path: &Path) -> Result<(), DevinError> {
    use std::os::unix::fs::PermissionsExt;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .await
        .map_err(|error| store_error("set permissions on", path, &error))
}

#[cfg(not(unix))]
async fn set_private_permissions(_path: &Path) -> Result<(), DevinError> {
    Ok(())
}
