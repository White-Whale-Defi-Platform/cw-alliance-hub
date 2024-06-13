use cosmwasm_std::{DepsMut, Order, Uint128};
use cw_asset::AssetInfo;
use cw_storage_plus::Map;

use crate::state::TOTAL_BALANCES;

pub(crate) fn migrate_state(mut deps: DepsMut) -> Result<(), String> {
    // const OLD_TOTAL_BALANCES: Map<&AssetInfo, Uint128> = Map::new("total_balances");
    //
    // let old_map = OLD_TOTAL_BALANCES
    //     .range(deps.storage, None, None, Order::Ascending)
    //     .map(|item| {
    //         let (key, value) = item.unwrap();
    //         (key, value)
    //     })
    //     .collect::<Vec<_>>();
    //
    // OLD_TOTAL_BALANCES.clear(deps.storage);
    //
    // for (key, value) in old_map {
    //     TOTAL_BALANCES
    //         .save(deps.storage, &key, &(value, value))?;
    // }

    Ok(())
}
