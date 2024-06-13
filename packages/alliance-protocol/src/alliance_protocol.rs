use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal, Timestamp, Uint128};
use cw20::Cw20ReceiveMsg;
use cw_asset::{Asset, AssetInfo};
use std::collections::{HashMap, HashSet};

#[cw_serde]
pub struct Config {
    pub governance: Addr,
    pub controller: Addr,
    pub oracle: Addr,
    pub operator: Addr,
    pub take_rate_taker: Addr,
    pub last_reward_update_timestamp: Timestamp,
    pub alliance_token_denom: String,
    pub alliance_token_supply: Uint128,
    pub reward_denom: String,
}

#[cw_serde]
pub struct AssetDistribution {
    pub asset: AssetInfo,
    pub distribution: Decimal,
}

#[cw_serde]
pub struct InstantiateMsg {
    pub governance: String,
    pub controller: String,
    pub alliance_token_denom: String,
    pub oracle: String,
    pub operator: String,
    pub take_rate_taker: String,
    pub reward_denom: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),

    // Public functions
    Stake {},
    Unstake(Asset),
    ClaimRewards(AssetInfo),
    UpdateRewards {},

    // Privileged functions

    // operator
    DistributeTakeRate {
        update: Option<bool>,
        assets: Option<Vec<AssetInfo>>,
    },

    WhitelistAssets(HashMap<ChainId, Vec<AssetInfo>>),
    RemoveAssets(Vec<AssetInfo>),
    UpdateRewardsCallback {},
    AllianceDelegate(AllianceDelegateMsg),
    AllianceUndelegate(AllianceUndelegateMsg),
    AllianceRedelegate(AllianceRedelegateMsg),
    RebalanceEmissions {},
    RebalanceEmissionsCallback {},
    SetAssetRewardDistribution(Vec<AssetDistribution>),
    UpdateConfig {
        governance: Option<String>,
        controller: Option<String>,
        oracle: Option<String>,
        operator: Option<String>,
        take_rate_taker: Option<String>,
    },
}

#[cw_serde]
pub enum Cw20HookMsg {
    Stake {},
    Unstake(Asset),
}

#[cw_serde]
pub struct AllianceDelegation {
    pub validator: String,
    pub amount: Uint128,
}

#[cw_serde]
pub struct AllianceDelegateMsg {
    pub delegations: Vec<AllianceDelegation>,
}

#[cw_serde]
pub struct AllianceUndelegateMsg {
    pub undelegations: Vec<AllianceDelegation>,
}

#[cw_serde]
pub struct AllianceRedelegation {
    pub src_validator: String,
    pub dst_validator: String,
    pub amount: Uint128,
}

#[cw_serde]
pub struct AllianceRedelegateMsg {
    pub redelegations: Vec<AllianceRedelegation>,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Config)]
    Config {},

    #[returns(HashSet<Addr>)]
    Validators {},

    #[returns(WhitelistedAssetsResponse)]
    WhitelistedAssets {},

    #[returns(Vec<AssetDistribution>)]
    RewardDistribution {},

    #[returns(StakedBalanceRes)]
    StakedBalance(AssetQuery),

    #[returns(PendingRewardsRes)]
    PendingRewards(AssetQuery),

    #[returns(Vec<StakedBalanceRes>)]
    AllStakedBalances(AllStakedBalancesQuery),

    #[returns(Vec<PendingRewardsRes>)]
    AllPendingRewards(AllPendingRewardsQuery),

    #[returns(Vec<StakedBalanceRes>)]
    TotalStakedBalances {},
}

pub type ChainId = String;
pub type WhitelistedAssetsResponse = HashMap<ChainId, Vec<AssetInfo>>;

#[cw_serde]
pub struct AssetQuery {
    pub address: String,
    pub asset: AssetInfo,
}

#[cw_serde]
pub struct AllStakedBalancesQuery {
    pub address: String,
}

#[cw_serde]
pub struct AllPendingRewardsQuery {
    pub address: String,
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
pub struct StakedBalanceRes {
    pub asset: AssetInfo,
    pub balance: Uint128,
}

#[cw_serde]
pub struct PendingRewardsRes {
    pub staked_asset: AssetInfo,
    pub reward_asset: AssetInfo,
    pub rewards: Uint128,
}
