use cosmwasm_std::{DecimalRangeExceeded, StdError};
use cw_asset_v3::AssetError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    DecimalRangeExceeded(#[from] DecimalRangeExceeded),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Custom Error val: {val:?}")]
    CustomError { val: String },
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
    #[error("Only a single asset is allowed")]
    OnlySingleAssetAllowed {},

    #[error("Asset not whitelisted")]
    AssetNotWhitelisted {},

    #[error("Insufficient balance")]
    InsufficientBalance {},

    #[error("Amount cannot be zero")]
    AmountCannotBeZero {},

    #[error("Invalid reply id {0}")]
    InvalidReplyId(u64),

    #[error("Empty delegation")]
    EmptyDelegation {},

    #[error("Invalid Distribution")]
    InvalidDistribution {},

    #[error("Semver parsing error: {0}")]
    SemVer(String),

    #[error("{0}")]
    AssetError(#[from] AssetError),
}

impl From<semver::Error> for ContractError {
    fn from(err: semver::Error) -> Self {
        Self::SemVer(err.to_string())
    }
}
