//! Canonical spreadsheet records, immutable commit replay, and safe adapters.
mod google;
mod journal;
mod legacy;
mod local_xlsx;
mod oauth;
pub use google::*;
pub use journal::*;
pub use legacy::*;
pub use local_xlsx::*;
pub use oauth::*;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Corrupt(String),
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Domain(#[from] portfolio_core::Error),
    #[error("{0}")]
    Remote(String),
}
pub type Result<T> = std::result::Result<T, Error>;

mod application;
pub use application::*;
mod market_csv;
pub use market_csv::*;
