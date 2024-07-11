use alliance_protocol::alliance_protocol::Config;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, DepsMut, Order, Timestamp, Uint128};
use cw_asset::AssetInfo;
use cw_storage_plus::{Item, Map};

use crate::error::ContractError;
use crate::state::{CONFIG, SHARES, TOTAL_BALANCES_SHARES};

pub(crate) fn migrate_state(deps: DepsMut) -> Result<(), ContractError> {
    #[cw_serde]
    struct OldConfig {
        pub governance: Addr,
        pub controller: Addr,
        pub oracle: Addr,
        pub operator: Addr,
        pub last_reward_update_timestamp: Timestamp,
        pub alliance_token_denom: String,
        pub alliance_token_supply: Uint128,
        pub reward_denom: String,
    }

    const OLD_CONFIG: Item<OldConfig> = Item::new("config");
    let old_config = OLD_CONFIG.load(deps.storage)?;

    let config = Config {
        governance: old_config.governance,
        controller: old_config.controller,
        oracle: old_config.oracle,
        operator: old_config.operator,
        take_rate_taker: Addr::unchecked(""),
        last_reward_update_timestamp: old_config.last_reward_update_timestamp,
        alliance_token_denom: old_config.alliance_token_denom,
        alliance_token_supply: old_config.alliance_token_supply,
        reward_denom: old_config.reward_denom,
        default_yearly_take_rate: Decimal::percent(5),
    };

    CONFIG.save(deps.storage, &config)?;

    const OLD_TOTAL_BALANCES: Map<&AssetInfo, Uint128> = Map::new("total_balances");

    let old_map = OLD_TOTAL_BALANCES
        .range(deps.storage, None, None, Order::Ascending)
        .map(|item| {
            let (key, value) = item.unwrap();
            (key, value)
        })
        .collect::<Vec<_>>();

    OLD_TOTAL_BALANCES.clear(deps.storage);

    for (key, value) in old_map {
        TOTAL_BALANCES_SHARES.save(deps.storage, &key, &(value, value))?;
    }

    const OLD_BALANCES: Map<(Addr, &AssetInfo), Uint128> = Map::new("balances");

    let old_map = OLD_BALANCES
        .range(deps.storage, None, None, Order::Ascending)
        .map(|item| {
            let (key, value) = item.unwrap();
            (key, value)
        })
        .collect::<Vec<_>>();

    OLD_BALANCES.clear(deps.storage);

    for ((addr, asset_info), value) in old_map {
        SHARES.save(deps.storage, (addr, &asset_info), &value)?;
    }

    Ok(())
}
