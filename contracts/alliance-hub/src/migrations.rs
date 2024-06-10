use crate::error::ContractError;
use crate::state::{
    ASSET_REWARD_DISTRIBUTION, ASSET_REWARD_RATE, BALANCES, TOTAL_BALANCES, UNCLAIMED_REWARDS,
    USER_ASSET_REWARD_RATE, WHITELIST,
};
use alliance_protocol::alliance_oracle_types::ChainId;
use alliance_protocol::alliance_protocol::AssetDistribution;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, DepsMut, Order, Uint128};
use cw_storage_plus_016::{Item as Item016, Map as Map016};

pub(crate) fn migrate_maps(mut deps: DepsMut) -> Result<(), ContractError> {
    migrate_whitelist_map(deps.branch())?;
    migrate_balances_map(deps.branch())?;
    migrate_total_balances_map(deps.branch())?;
    migrate_asset_reward_distribution(deps.branch())?;
    migrate_asset_reward_rate(deps.branch())?;
    migrate_user_asset_reward_rate(deps.branch())?;
    migrate_unclaimed_rewards(deps.branch())?;

    Ok(())
}

fn migrate_whitelist_map(deps: DepsMut) -> Result<(), ContractError> {
    const OLD_WHITELIST: Map016<cw_asset_v2::AssetInfoKey, ChainId> = Map016::new("whitelist");

    let old_map = OLD_WHITELIST
        .range(deps.storage, None, None, Order::Ascending)
        .into_iter()
        .map(|item| {
            let (key, value) = item.unwrap();
            (key, value)
        })
        .collect::<Vec<_>>();

    OLD_WHITELIST.clear(deps.storage);

    for (key, value) in old_map {
        let asset_info_v3 = match key {
            cw_asset_v2::AssetInfoUnchecked::Native(x) => cw_asset_v3::AssetInfo::native(x),
            cw_asset_v2::AssetInfoUnchecked::Cw20(x) => {
                cw_asset_v3::AssetInfo::cw20(deps.api.addr_validate(&x)?)
            }
            _ => panic!("unsupported"),
        };
        WHITELIST
            .save(deps.storage, &asset_info_v3, &value)
            .unwrap();
    }

    Ok(())
}
fn migrate_balances_map(deps: DepsMut) -> Result<(), ContractError> {
    const OLD_BALANCES: Map016<(Addr, cw_asset_v2::AssetInfoKey), Uint128> =
        Map016::new("balances");

    let old_map = OLD_BALANCES
        .range(deps.storage, None, None, Order::Ascending)
        .into_iter()
        .map(|item| {
            let (key, value) = item.unwrap();
            (key, value)
        })
        .collect::<Vec<_>>();

    OLD_BALANCES.clear(deps.storage);

    for (key, value) in old_map {
        let asset_info_v3 = match key.1 {
            cw_asset_v2::AssetInfoUnchecked::Native(x) => cw_asset_v3::AssetInfo::native(x),
            cw_asset_v2::AssetInfoUnchecked::Cw20(x) => {
                cw_asset_v3::AssetInfo::cw20(deps.api.addr_validate(&x)?)
            }
            _ => panic!("unsupported"),
        };

        BALANCES
            .save(deps.storage, (key.0, &asset_info_v3), &value)
            .unwrap();
    }

    Ok(())
}
fn migrate_total_balances_map(deps: DepsMut) -> Result<(), ContractError> {
    const OLD_TOTAL_BALANCES: Map016<cw_asset_v2::AssetInfoKey, Uint128> =
        Map016::new("total_balances");

    let old_map = OLD_TOTAL_BALANCES
        .range(deps.storage, None, None, Order::Ascending)
        .into_iter()
        .map(|item| {
            let (key, value) = item.unwrap();
            (key, value)
        })
        .collect::<Vec<_>>();

    OLD_TOTAL_BALANCES.clear(deps.storage);

    for (key, value) in old_map {
        let asset_info_v3 = match key {
            cw_asset_v2::AssetInfoUnchecked::Native(x) => cw_asset_v3::AssetInfo::native(x),
            cw_asset_v2::AssetInfoUnchecked::Cw20(x) => {
                cw_asset_v3::AssetInfo::cw20(deps.api.addr_validate(&x)?)
            }
            _ => panic!("unsupported"),
        };

        TOTAL_BALANCES
            .save(deps.storage, &asset_info_v3, &value)
            .unwrap();
    }

    Ok(())
}

fn migrate_asset_reward_distribution(deps: DepsMut) -> Result<(), ContractError> {
    #[cw_serde]
    pub struct OldAssetDistribution {
        pub asset: cw_asset_v2::AssetInfo,
        pub distribution: Decimal,
    }

    const OLD_ASSET_REWARD_DISTRIBUTION: Item016<Vec<OldAssetDistribution>> =
        Item016::new("asset_reward_distribution");
    let old_asset_reward_distribution = OLD_ASSET_REWARD_DISTRIBUTION.load(deps.storage)?;

    OLD_ASSET_REWARD_DISTRIBUTION.remove(deps.storage);

    let mut asset_reward_distribution: Vec<AssetDistribution> = Vec::new();
    for a in old_asset_reward_distribution {
        let asset_info_v3 = match a.asset {
            cw_asset_v2::AssetInfo::Native(x) => cw_asset_v3::AssetInfo::native(x),
            cw_asset_v2::AssetInfo::Cw20(x) => cw_asset_v3::AssetInfo::cw20(x),
            _ => panic!("unsupported"),
        };

        asset_reward_distribution.push(AssetDistribution {
            asset: asset_info_v3,
            distribution: a.distribution,
        });
    }

    ASSET_REWARD_DISTRIBUTION.save(deps.storage, &asset_reward_distribution)?;

    Ok(())
}

fn migrate_asset_reward_rate(deps: DepsMut) -> Result<(), ContractError> {
    const OLD_ASSET_REWARD_RATE: Map016<cw_asset_v2::AssetInfoKey, Decimal> =
        Map016::new("asset_reward_rate");

    let old_map = OLD_ASSET_REWARD_RATE
        .range(deps.storage, None, None, Order::Ascending)
        .into_iter()
        .map(|item| {
            let (key, value) = item.unwrap();
            (key, value)
        })
        .collect::<Vec<_>>();

    OLD_ASSET_REWARD_RATE.clear(deps.storage);

    for (key, value) in old_map {
        let asset_info_v3 = match key {
            cw_asset_v2::AssetInfoUnchecked::Native(x) => cw_asset_v3::AssetInfo::native(x),
            cw_asset_v2::AssetInfoUnchecked::Cw20(x) => {
                cw_asset_v3::AssetInfo::cw20(deps.api.addr_validate(&x)?)
            }
            _ => panic!("unsupported"),
        };

        ASSET_REWARD_RATE
            .save(deps.storage, &asset_info_v3, &value)
            .unwrap();
    }

    Ok(())
}

fn migrate_user_asset_reward_rate(deps: DepsMut) -> Result<(), ContractError> {
    const OLD_USER_ASSET_REWARD_RATE: Map016<(Addr, cw_asset_v2::AssetInfoKey), Decimal> =
        Map016::new("user_asset_reward_rate");

    let old_map = OLD_USER_ASSET_REWARD_RATE
        .range(deps.storage, None, None, Order::Ascending)
        .into_iter()
        .map(|item| {
            let (key, value) = item.unwrap();
            (key, value)
        })
        .collect::<Vec<_>>();

    OLD_USER_ASSET_REWARD_RATE.clear(deps.storage);

    for (key, value) in old_map {
        let asset_info_v3 = match key.1 {
            cw_asset_v2::AssetInfoUnchecked::Native(x) => cw_asset_v3::AssetInfo::native(x),
            cw_asset_v2::AssetInfoUnchecked::Cw20(x) => {
                cw_asset_v3::AssetInfo::cw20(deps.api.addr_validate(&x)?)
            }
            _ => panic!("unsupported"),
        };

        USER_ASSET_REWARD_RATE
            .save(deps.storage, (key.0, &asset_info_v3), &value)
            .unwrap();
    }

    Ok(())
}

fn migrate_unclaimed_rewards(deps: DepsMut) -> Result<(), ContractError> {
    pub const OLD_UNCLAIMED_REWARDS: Map016<(Addr, cw_asset_v2::AssetInfoKey), Uint128> =
        Map016::new("unclaimed_rewards");

    let old_map = OLD_UNCLAIMED_REWARDS
        .range(deps.storage, None, None, Order::Ascending)
        .into_iter()
        .map(|item| {
            let (key, value) = item.unwrap();
            (key, value)
        })
        .collect::<Vec<_>>();

    OLD_UNCLAIMED_REWARDS.clear(deps.storage);

    for (key, value) in old_map {
        let asset_info_v3 = match key.1 {
            cw_asset_v2::AssetInfoUnchecked::Native(x) => cw_asset_v3::AssetInfo::native(x),
            cw_asset_v2::AssetInfoUnchecked::Cw20(x) => {
                cw_asset_v3::AssetInfo::cw20(deps.api.addr_validate(&x)?)
            }
            _ => panic!("unsupported"),
        };

        UNCLAIMED_REWARDS
            .save(deps.storage, (key.0, &asset_info_v3), &value)
            .unwrap();
    }

    Ok(())
}
