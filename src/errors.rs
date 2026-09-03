use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::dex::DexKind;

#[derive(Debug, Clone, Copy)]
pub enum ErrorSeverity {
    Info,
    Warning,
    Critical,
}

impl ErrorSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MonitorErrorRecord {
    pub occurred_at: DateTime<Utc>,
    pub component: String,
    pub severity: ErrorSeverity,
    pub message: String,
    pub source: Option<String>,
    pub dex: Option<DexKind>,
    pub pool_address: Option<String>,
    pub retry_planned: bool,
    pub consecutive_count: u32,
}

impl MonitorErrorRecord {
    pub fn new(
        component: impl Into<String>,
        severity: ErrorSeverity,
        message: impl Into<String>,
        source: Option<String>,
    ) -> Self {
        Self {
            occurred_at: Utc::now(),
            component: component.into(),
            severity,
            message: message.into(),
            source,
            dex: None,
            pool_address: None,
            retry_planned: true,
            consecutive_count: 1,
        }
    }

    pub fn with_pool_context(mut self, dex: DexKind, pool_address: impl Into<String>) -> Self {
        self.dex = Some(dex);
        self.pool_address = Some(pool_address.into());
        self
    }

    pub fn with_retry_planned(mut self, retry_planned: bool) -> Self {
        self.retry_planned = retry_planned;
        self
    }

    pub fn with_consecutive_count(mut self, consecutive_count: u32) -> Self {
        self.consecutive_count = consecutive_count.max(1);
        self
    }
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("rpc error: {0}")]
    Rpc(String),
    #[error("decode error: {0}")]
    Decode(String),
    #[error("pricing error: {0}")]
    Pricing(String),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("notification error: {0}")]
    Notification(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl AppError {
    pub fn component(&self) -> &'static str {
        match self {
            Self::Config(_) | Self::Io(_) | Self::Toml(_) | Self::Json(_) => "config",
            Self::Rpc(_) => "rpc",
            Self::Decode(_) => "decode",
            Self::Pricing(_) => "pricing",
            Self::Database(_) => "database",
            Self::Notification(_) => "notification",
        }
    }

    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::Config(_) | Self::Database(_) => ErrorSeverity::Critical,
            Self::Rpc(_) | Self::Decode(_) | Self::Pricing(_) | Self::Notification(_) => {
                ErrorSeverity::Warning
            }
            Self::Io(_) | Self::Toml(_) | Self::Json(_) => ErrorSeverity::Critical,
        }
    }

    pub fn to_monitor_record(&self) -> MonitorErrorRecord {
        MonitorErrorRecord::new(
            self.component(),
            self.severity(),
            self.to_string(),
            std::error::Error::source(self).map(ToString::to_string),
        )
    }
}
