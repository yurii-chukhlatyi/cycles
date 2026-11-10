//! Shared accounting engine. Stored money is decimal and the UI never calculates ledgers.
mod decimal;
mod ledger;
mod performance;
mod records;
mod reporting;
mod settlement;
mod validation;
pub use ledger::*;
pub use performance::*;
pub use records::*;
pub use reporting::*;
pub use settlement::*;
pub use validation::*;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("The value exceeds supported decimal precision")]
    Overflow,
    #[error("{0}")]
    Serialization(#[from] serde_json::Error),
}
pub type Result<T> = std::result::Result<T, Error>;

mod attribution;
pub use attribution::*;

mod consolidation;
pub use consolidation::*;
