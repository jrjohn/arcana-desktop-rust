//! Hand-built object graph (C# `ServiceCollectionExtensions.AddArcanaServices` minus plugins).
//!
//! C# also registers `SyncService` as itself (no implementation exists); a
//! [`arcana_sync::QueueSyncService`] needs a transport, so the app constructs it explicitly
//! once one exists.

use std::path::PathBuf;
use std::sync::Arc;

use arcana_core::security::{
    AuthService, AuthorizationService, CurrentUserService, PasswordHasher, TokenService,
};
use arcana_core::{AppError, AppResult, ErrorCode};
use arcana_data::{CustomerRepositoryImpl, Db, OrderRepositoryImpl, ProductRepositoryImpl};
use arcana_domain::services::{CustomerService, OrderService, ProductService};
use arcana_domain::{CustomerValidator, OrderValidator, ProductValidator};

use crate::localization::{LocalizationService, LocalizationServiceImpl};
use crate::security::{
    AuditLogger, AuthServiceImpl, AuthorizationServiceImpl, CurrentUserServiceImpl,
    PasswordHasherImpl, TokenServiceImpl, TokenServiceOptions,
};
use crate::services::{CustomerServiceImpl, OrderServiceImpl, ProductServiceImpl};
use crate::settings::{AppSettingsService, JsonSettingsService, SettingsService, app_data_dir};

/// The C# database location `<local app data>/Arcana/data/arcana.db` as an sqlx URL, creating
/// the directory.
///
/// # Errors
/// `FileAccessDenied` when the directory cannot be created.
pub fn default_database_url() -> AppResult<String> {
    let dir = app_data_dir().join("data");
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::data(ErrorCode::FileAccessDenied, e.to_string()))?;
    Ok(sqlite_url(&dir.join("arcana.db")))
}

/// sqlx `SQLite` URL for a database file. Windows path separators become `/` so the URL has one
/// shape on every OS (`sqlite://C:/Users/.../arcana.db`); a raw `\` path only worked because
/// sqlx happens to pass the remainder through as a file name.
#[must_use]
pub fn sqlite_url(file: &std::path::Path) -> String {
    format!("sqlite://{}", file.display().to_string().replace('\\', "/"))
}

/// Inputs for [`ArcanaServices::new`].
#[derive(Clone)]
pub struct ArcanaConfig {
    /// HMAC key for access tokens (C# `Security:Token:SecretKey`). Never hard-code it.
    pub token_key: Vec<u8>,
    pub token_options: TokenServiceOptions,
    /// Key/value settings file.
    pub settings_path: PathBuf,
    /// Typed app settings file (C# `settings.json`).
    pub app_settings_path: PathBuf,
}

impl std::fmt::Debug for ArcanaConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArcanaConfig")
            .field("token_key", &"<redacted>")
            .field("token_options", &self.token_options)
            .field("settings_path", &self.settings_path)
            .field("app_settings_path", &self.app_settings_path)
            .finish()
    }
}

/// Every infrastructure service, wired on one database.
#[derive(Clone)]
pub struct ArcanaServices {
    pub db: Db,
    pub password_hasher: Arc<dyn PasswordHasher>,
    pub token_service: Arc<dyn TokenService>,
    pub current_user: Arc<dyn CurrentUserService>,
    pub auth: Arc<dyn AuthService>,
    pub authorization: Arc<dyn AuthorizationService>,
    pub audit: AuditLogger,
    pub customers: Arc<dyn CustomerService>,
    pub products: Arc<dyn ProductService>,
    pub orders: Arc<dyn OrderService>,
    pub settings: Arc<dyn SettingsService>,
    pub app_settings: Arc<AppSettingsService>,
    pub localization: Arc<dyn LocalizationService>,
}

impl std::fmt::Debug for ArcanaServices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ArcanaServices")
    }
}

impl ArcanaServices {
    /// Connect (creating + migrating) the database at `url` and wire everything on it.
    ///
    /// # Errors
    /// Connection/migration failure, or an empty token key.
    pub async fn open(url: &str, config: ArcanaConfig) -> AppResult<Self> {
        Self::new(Db::connect(url).await?, config)
    }

    /// Wire everything on a fresh in-memory database (tests, previews, screenshots).
    ///
    /// # Errors
    /// Database creation/migration failure, or an empty token key.
    pub async fn in_memory(config: ArcanaConfig) -> AppResult<Self> {
        Self::new(Db::in_memory().await?, config)
    }

    /// Wire everything on `db`.
    ///
    /// # Errors
    /// `ValidationFailed` when the token key is empty.
    pub fn new(db: Db, config: ArcanaConfig) -> AppResult<Self> {
        let password_hasher: Arc<dyn PasswordHasher> = Arc::new(PasswordHasherImpl::new());
        let token_service: Arc<dyn TokenService> = Arc::new(TokenServiceImpl::new(
            &config.token_key,
            config.token_options,
        )?);
        let current_user: Arc<dyn CurrentUserService> = Arc::new(CurrentUserServiceImpl::new());
        let auth = Arc::new(AuthServiceImpl::new(
            db.clone(),
            password_hasher.clone(),
            token_service.clone(),
            current_user.clone(),
        ));
        Ok(Self {
            authorization: Arc::new(AuthorizationServiceImpl::new(db.clone())),
            audit: AuditLogger::new(db.clone()),
            customers: Arc::new(CustomerServiceImpl::new(
                Arc::new(CustomerRepositoryImpl::with_db(db.clone())),
                Arc::new(CustomerValidator),
            )),
            products: Arc::new(ProductServiceImpl::new(
                Arc::new(ProductRepositoryImpl::with_db(db.clone())),
                Arc::new(ProductValidator),
            )),
            orders: Arc::new(OrderServiceImpl::new(
                Arc::new(OrderRepositoryImpl::with_db(db.clone())),
                Arc::new(OrderValidator),
            )),
            settings: Arc::new(JsonSettingsService::new(config.settings_path)),
            app_settings: Arc::new(AppSettingsService::new(config.app_settings_path)),
            localization: Arc::new(LocalizationServiceImpl::new()),
            auth,
            password_hasher,
            token_service,
            current_user,
            db,
        })
    }
}
