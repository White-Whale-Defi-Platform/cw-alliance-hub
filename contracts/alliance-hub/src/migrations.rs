use cosmwasm_std::{Addr, DepsMut, Order, Uint128};
use cw_asset::AssetInfo;
use cw_storage_plus::Map;

use crate::error::ContractError;
use crate::state::{SHARES, TOTAL_BALANCES_SHARES};

pub(crate) fn migrate_state(deps: DepsMut) -> Result<(), ContractError> {
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
