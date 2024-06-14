use alliance_protocol::alliance_protocol::{AssetDistribution, ChainId, Config};
use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_asset::AssetInfo;
use cw_storage_plus::{Item, Map};
use std::collections::HashSet;
use ve3_shared::msgs_asset_staking::AssetConfigRuntime;

pub const CONFIG: Item<Config> = Item::new("config");
pub const WHITELIST: Map<&AssetInfo, ChainId> = Map::new("whitelist");
pub const SHARES: Map<(Addr, &AssetInfo), Uint128> = Map::new("shares");
pub const TOTAL_BALANCES_SHARES: Map<&AssetInfo, (Uint128, Uint128)> =
    Map::new("total_balances_shares");
pub const VALIDATORS: Item<HashSet<String>> = Item::new("validators");
pub const ASSET_CONFIG: Map<&AssetInfo, AssetConfigRuntime> = Map::new("asset_config");
pub const ASSET_REWARD_DISTRIBUTION: Item<Vec<AssetDistribution>> =
    Item::new("asset_reward_distribution");
pub const ASSET_REWARD_RATE: Map<&AssetInfo, Decimal> = Map::new("asset_reward_rate");
pub const USER_ASSET_REWARD_RATE: Map<(Addr, &AssetInfo), Decimal> =
    Map::new("user_asset_reward_rate");
pub const UNCLAIMED_REWARDS: Map<(Addr, &AssetInfo), Uint128> = Map::new("unclaimed_rewards");
pub const TEMP_BALANCE: Item<Uint128> = Item::new("temp_balance");
