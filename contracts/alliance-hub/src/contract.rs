#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use std::borrow::Borrow;

use alliance_protocol::alliance_protocol::{
    AllianceDelegateMsg, AllianceRedelegateMsg, AllianceUndelegateMsg, ExecuteMsg, InstantiateMsg,
    QueryMsg,
};
use cosmwasm_std::CosmosMsg::Custom;
use cosmwasm_std::{
    coin, to_binary, Addr, Binary, Coin as CwCoin, CosmosMsg, Decimal, Deps, DepsMut, Empty, Env,
    MessageInfo, Reply, Response, StdError, StdResult, Storage, SubMsg, Timestamp, Uint128,
    WasmMsg,
};
use cw2::set_contract_version;
use cw_asset::{Asset, AssetInfo, AssetInfoKey};
use cw_utils::parse_instantiate_response_data;
use std::collections::HashSet;

use terra_proto_rs::alliance::alliance::{
    MsgClaimDelegationRewards, MsgDelegate, MsgRedelegate, MsgUndelegate, Redelegation,
};
use terra_proto_rs::cosmos::base::v1beta1::Coin;
use terra_proto_rs::cosmos::distribution::v1beta1::MsgWithdrawDelegatorReward;
use terra_proto_rs::traits::Message;

use crate::error::ContractError;
use crate::error::ContractError::DecimalRangeExceeded;
use crate::state::{
    Config, ASSET_REWARD_DISTRIBUTION, ASSET_REWARD_RATE, BALANCES, CONFIG, TEMP_BALANCE,
    TOTAL_BALANCES, USER_ASSET_REWARD_RATE, VALIDATORS, WHITELIST,
};
use crate::token_factory::{CustomExecuteMsg, DenomUnit, Metadata, TokenExecuteMsg};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:terra-alliance-protocol";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const CREATE_REPLY_ID: u64 = 1;
const CLAIM_REWARD_REPLY_ID: u64 = 2;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response<CustomExecuteMsg>, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let governance_address = deps.api.addr_validate(msg.governance_address.as_str())?;
    let controller_address = deps.api.addr_validate(msg.controller_address.as_str())?;
    let denom = "ualliance";
    let symbol = "ALLIANCE";
    let create_msg = TokenExecuteMsg::CreateDenom {
        subdenom: denom.to_string(),
        metadata: Metadata {
            description: "Staking token for the alliance protocol".to_string(),
            denom_units: vec![DenomUnit {
                denom: "ualliance".to_string(),
                exponent: 0,
                aliases: vec![],
            }],
            base: denom.to_string(),
            display: symbol.to_string(),
            name: "Alliance Token".to_string(),
            symbol: symbol.to_string(),
        },
    };
    let sub_msg = SubMsg::reply_on_success(
        CosmosMsg::Custom(CustomExecuteMsg::Token(create_msg)),
        CREATE_REPLY_ID,
    );
    let config = Config {
        governance_address,
        controller_address,
        alliance_token_denom: "".to_string(),
        alliance_token_supply: Uint128::zero(),
        last_reward_update_timestamp: Timestamp::default(),
        reward_denom: msg.reward_denom,
    };
    CONFIG.save(deps.storage, &config)?;

    VALIDATORS.save(deps.storage, &HashSet::new())?;
    Ok(Response::new()
        .add_attributes(vec![("action", "instantiate")])
        .add_submessage(sub_msg))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::WhitelistAssets(assets) => whitelist_assets(deps, env, info, assets),
        ExecuteMsg::RemoveAssets(assets) => remove_assets(deps, env, info, assets),

        ExecuteMsg::Stake => stake(deps, env, info),
        ExecuteMsg::Unstake(asset) => unstake(deps, env, info, asset),
        ExecuteMsg::ClaimRewards(asset) => claim_rewards(deps, env, info, asset),

        ExecuteMsg::AllianceDelegate(msg) => alliance_delegate(deps, env, info, msg),
        ExecuteMsg::AllianceUndelegate(msg) => alliance_undelegate(deps, env, info, msg),
        ExecuteMsg::AllianceRedelegate(msg) => alliance_redelegate(deps, env, info, msg),
        ExecuteMsg::UpdateRewards => update_rewards(deps, env, info),
        ExecuteMsg::RebalanceEmissions => Ok(Response::new()),

        ExecuteMsg::UpdateRewardsCallback => update_reward_callback(deps, env, info),
    }
}

fn whitelist_assets(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    assets: Vec<AssetInfo>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.governance_address {
        return Err(ContractError::Unauthorized {});
    }
    for asset in &assets {
        let asset_key = AssetInfoKey::from(asset.clone());
        WHITELIST.save(deps.storage, asset_key, &true)?;
    }
    let assets_str = assets
        .iter()
        .map(|asset| asset.to_string())
        .collect::<Vec<String>>()
        .join(",");
    Ok(Response::new().add_attributes(vec![
        ("action", "whitelist_assets"),
        ("assets", &assets_str),
    ]))
}

fn remove_assets(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    assets: Vec<AssetInfo>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.governance_address {
        return Err(ContractError::Unauthorized {});
    }
    for asset in &assets {
        let asset_key = AssetInfoKey::from(asset.clone());
        WHITELIST.remove(deps.storage, asset_key);
    }
    let assets_str = assets
        .iter()
        .map(|asset| asset.to_string())
        .collect::<Vec<String>>()
        .join(",");
    Ok(Response::new().add_attributes(vec![("action", "remove_assets"), ("assets", &assets_str)]))
}

fn stake(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    if info.funds.len() != 1 {
        return Err(ContractError::OnlySingleAssetAllowed {});
    }
    if info.funds[0].amount.is_zero() {
        return Err(ContractError::AmountCannotBeZero {});
    }
    let asset = AssetInfo::native(&info.funds[0].denom);
    let asset_key = AssetInfoKey::from(&asset);
    let whitelisted = WHITELIST
        .load(deps.storage, asset_key.clone())
        .unwrap_or(false);
    if !whitelisted {
        return Err(ContractError::AssetNotWhitelisted {});
    }
    let sender = info.sender.clone();

    // TODO: Before updating the balance, we need to calculate of amount of rewards accured for this user
    BALANCES.update(
        deps.storage,
        (sender.clone(), asset_key.clone()),
        |balance| -> Result<_, ContractError> {
            match balance {
                Some(balance) => Ok(balance + info.funds[0].amount),
                None => Ok(info.funds[0].amount),
            }
        },
    )?;
    TOTAL_BALANCES.update(
        deps.storage,
        asset_key.clone(),
        |balance| -> Result<_, ContractError> {
            Ok(balance.unwrap_or(Uint128::zero()) + info.funds[0].amount)
        },
    )?;

    let asset_reward_rate = ASSET_REWARD_RATE
        .load(deps.storage, asset_key.clone())
        .unwrap_or(Decimal::zero());
    USER_ASSET_REWARD_RATE.save(
        deps.storage,
        (sender.clone(), asset_key.clone()),
        &asset_reward_rate,
    )?;

    Ok(Response::new().add_attributes(vec![
        ("action", "stake"),
        ("user", &info.sender.to_string()),
        ("asset", &asset.to_string()),
        ("amount", &info.funds[0].amount.to_string()),
    ]))
}

fn unstake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    asset: Asset,
) -> Result<Response, ContractError> {
    let asset_key = AssetInfoKey::from(asset.info.clone());
    let sender = info.sender.clone();
    if asset.amount.is_zero() {
        return Err(ContractError::AmountCannotBeZero {});
    }

    // TODO: Calculate rewards accured and claim it

    BALANCES.update(
        deps.storage,
        (sender, asset_key.clone()),
        |balance| -> Result<_, ContractError> {
            match balance {
                Some(balance) => {
                    if balance < asset.amount {
                        return Err(ContractError::InsufficientBalance {});
                    }
                    Ok(balance - asset.amount)
                }
                None => Err(ContractError::InsufficientBalance {}),
            }
        },
    )?;
    TOTAL_BALANCES.update(
        deps.storage,
        asset_key.clone(),
        |balance| -> Result<_, ContractError> {
            let balance = balance.unwrap_or(Uint128::zero());
            if balance < asset.amount {
                return Err(ContractError::InsufficientBalance {});
            }
            Ok(balance - asset.amount)
        },
    )?;

    let msg = asset.transfer_msg(&info.sender)?;

    Ok(Response::new()
        .add_attributes(vec![
            ("action", "unstake"),
            ("user", &info.sender.to_string()),
            ("asset", &asset.info.to_string()),
            ("amount", &asset.amount.to_string()),
        ])
        .add_message(msg))
}

fn claim_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    asset: AssetInfo,
) -> Result<Response, ContractError> {
    let user = info.sender.clone();
    let config = CONFIG.load(deps.storage)?;
    let rewards = _claim_reward(deps.storage, user.clone(), asset.clone())?;
    let response = Response::new().add_attributes(vec![
        ("action", "claim_rewards"),
        ("user", &user.to_string()),
        ("asset", &asset.to_string()),
        ("reward_amount", &rewards.to_string()),
    ]);
    if !rewards.is_zero() {
        let rewards_asset = Asset {
            info: AssetInfo::Native(config.reward_denom),
            amount: rewards,
        };
        Ok(response.add_message(rewards_asset.transfer_msg(&user)?))
    } else {
        Ok(response)
    }
}

fn _claim_reward(
    storage: &mut dyn Storage,
    user: Addr,
    asset: AssetInfo,
) -> Result<Uint128, ContractError> {
    let asset_key = AssetInfoKey::from(&asset);
    let user_reward_rate =
        USER_ASSET_REWARD_RATE.load(storage, (user.clone(), asset_key.clone()))?;
    let asset_reward_rate = ASSET_REWARD_RATE.load(storage, asset_key.clone())?;
    let user_staked = BALANCES.load(storage, (user.clone(), asset_key.clone()))?;
    let rewards = ((asset_reward_rate - user_reward_rate) * Decimal::from_atomics(user_staked, 0)?)
        .to_uint_floor();
    if rewards.is_zero() {
        Ok(Uint128::zero())
    } else {
        USER_ASSET_REWARD_RATE.save(
            storage,
            (user.clone(), asset_key.clone()),
            &asset_reward_rate,
        )?;
        Ok(rewards)
    }
}

fn alliance_delegate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: AllianceDelegateMsg,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.controller_address {
        return Err(ContractError::Unauthorized {});
    }
    if msg.delegations.is_empty() {
        return Err(ContractError::EmptyDelegation {});
    }
    let mut validators = VALIDATORS.load(deps.storage)?;
    let mut msgs: Vec<CosmosMsg<Empty>> = vec![];
    for delegation in msg.delegations {
        let validator = deps.api.addr_validate(&delegation.validator)?;
        let delegate_msg = MsgDelegate {
            amount: Some(Coin {
                denom: config.alliance_token_denom.clone(),
                amount: delegation.amount.to_string(),
            }),
            delegator_address: env.contract.address.to_string(),
            validator_address: validator.to_string(),
        };
        msgs.push(CosmosMsg::Stargate {
            type_url: "/alliance.alliance.MsgDelegate".to_string(),
            value: Binary::from(delegate_msg.encode_to_vec()),
        });
        validators.insert(validator);
    }
    VALIDATORS.save(deps.storage, &validators)?;
    Ok(Response::new()
        .add_attributes(vec![("action", "alliance_delegate")])
        .add_messages(msgs))
}

fn alliance_undelegate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: AllianceUndelegateMsg,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.controller_address {
        return Err(ContractError::Unauthorized {});
    }
    if msg.undelegations.is_empty() {
        return Err(ContractError::EmptyDelegation {});
    }
    let mut msgs = vec![];
    for delegation in msg.undelegations {
        let undelegate_msg = MsgUndelegate {
            amount: Some(Coin {
                denom: config.alliance_token_denom.clone(),
                amount: delegation.amount.to_string(),
            }),
            delegator_address: env.contract.address.to_string(),
            validator_address: delegation.validator.to_string(),
        };
        let msg = CosmosMsg::Stargate {
            type_url: "/alliance.alliance.MsgUndelegate".to_string(),
            value: Binary::from(undelegate_msg.encode_to_vec()),
        };
        msgs.push(msg);
    }
    Ok(Response::new()
        .add_attributes(vec![("action", "alliance_undelegate")])
        .add_messages(msgs))
}

fn alliance_redelegate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: AllianceRedelegateMsg,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.controller_address {
        return Err(ContractError::Unauthorized {});
    }
    if msg.redelegations.is_empty() {
        return Err(ContractError::EmptyDelegation {});
    }
    let mut msgs = vec![];
    let mut validators = VALIDATORS.load(deps.storage)?;
    for redelegation in msg.redelegations {
        let src_validator = deps.api.addr_validate(&redelegation.src_validator)?;
        let dst_validator = deps.api.addr_validate(&redelegation.dst_validator)?;
        let redelegate_msg = MsgRedelegate {
            amount: Some(Coin {
                denom: config.alliance_token_denom.clone(),
                amount: redelegation.amount.to_string(),
            }),
            delegator_address: env.contract.address.to_string(),
            validator_src_address: src_validator.to_string(),
            validator_dst_address: dst_validator.to_string(),
        };
        let msg = CosmosMsg::Stargate {
            type_url: "/alliance.alliance.MsgRedelegate".to_string(),
            value: Binary::from(redelegate_msg.encode_to_vec()),
        };
        msgs.push(msg);
        validators.insert(dst_validator);
    }
    VALIDATORS.save(deps.storage, &validators)?;
    Ok(Response::new()
        .add_attributes(vec![("action", "alliance_redelegate")])
        .add_messages(msgs))
}

fn update_rewards(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let reward_sent_in_tx: Option<&CwCoin> =
        info.funds.iter().find(|c| c.denom == config.reward_denom);
    let sent_balance = if let Some(coin) = reward_sent_in_tx {
        coin.amount
    } else {
        Uint128::zero()
    };
    let reward_asset = AssetInfo::native(config.reward_denom.clone());
    let contract_balance =
        reward_asset.query_balance(&deps.querier, env.contract.address.clone())?;

    // Contract balance is guaranteed to be greater than sent balance
    TEMP_BALANCE.save(deps.storage, &(contract_balance - sent_balance))?;
    let validators = VALIDATORS.load(deps.storage)?;
    let sub_msgs: Vec<SubMsg> = validators
        .iter()
        .map(|v| {
            let msg = MsgClaimDelegationRewards {
                delegator_address: env.contract.address.to_string(),
                validator_address: v.to_string(),
                denom: config.alliance_token_denom.clone(),
            };
            let msg = CosmosMsg::Stargate {
                type_url: "/alliance.alliance.MsgWithdrawDelegatorReward".to_string(),
                value: Binary::from(msg.encode_to_vec()),
            };
            SubMsg::new(msg)
        })
        .collect();
    let msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.to_string(),
        msg: to_binary(&ExecuteMsg::UpdateRewardsCallback).unwrap(),
        funds: vec![],
    });

    Ok(Response::new()
        .add_attributes(vec![("action", "update_rewards")])
        .add_submessages(sub_msgs)
        .add_message(msg))
}

fn update_reward_callback(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized {});
    }
    let config = CONFIG.load(deps.storage)?;
    let reward_asset = AssetInfo::native(config.reward_denom.clone());
    let current_balance =
        reward_asset.query_balance(&deps.querier, env.contract.address.clone())?;
    let previous_balance = TEMP_BALANCE.load(deps.storage)?;
    let rewards_collected = current_balance - previous_balance;

    let asset_reward_distribution = ASSET_REWARD_DISTRIBUTION.load(deps.storage)?;
    let total_distribution = asset_reward_distribution
        .iter()
        .map(|a| a.distribution)
        .fold(Decimal::zero(), |acc, v| acc + v);

    for asset_distribution in asset_reward_distribution {
        let asset_key = AssetInfoKey::from(asset_distribution.asset);
        let total_reward_distributed = Decimal::from_atomics(rewards_collected, 0)?
            * asset_distribution.distribution
            / total_distribution;

        // If there are no balances, we stop updating the rate. This means that the emissions are not directed to any stakers.
        let total_balance = TOTAL_BALANCES
            .load(deps.storage, asset_key.clone())
            .unwrap_or(Uint128::zero());
        if !total_balance.is_zero() {
            let rate_to_update =
                total_reward_distributed / Decimal::from_atomics(total_balance, 0)?;
            if rate_to_update > Decimal::zero() {
                ASSET_REWARD_RATE.update(
                    deps.storage,
                    asset_key.clone(),
                    |rate| -> StdResult<_> { Ok(rate.unwrap_or(Decimal::zero()) + rate_to_update) },
                )?;
            }
        }
    }
    TEMP_BALANCE.remove(deps.storage);

    Ok(Response::new().add_attributes(vec![("action", "update_rewards_callback")]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(
    deps: DepsMut,
    env: Env,
    reply: Reply,
) -> Result<Response<CustomExecuteMsg>, ContractError> {
    match reply.id {
        CREATE_REPLY_ID => {
            let response = reply.result.unwrap();
            // It works because the response data is a protobuf encoded string that contains the denom in the first slot (similar to the contract instantiation response)
            let denom = parse_instantiate_response_data(response.data.unwrap().as_slice())
                .map_err(|_| ContractError::Std(StdError::generic_err("parse error".to_string())))?
                .contract_address;
            let total_supply = Uint128::from(1000_000_000_000u128);
            let sub_msg = SubMsg::new(CosmosMsg::Custom(CustomExecuteMsg::Token(
                TokenExecuteMsg::MintTokens {
                    denom: denom.clone(),
                    amount: total_supply.clone(),
                    mint_to_address: env.contract.address.to_string(),
                },
            )));
            CONFIG.update(deps.storage, |mut config| -> Result<_, ContractError> {
                config.alliance_token_denom = denom.clone();
                config.alliance_token_supply = total_supply.clone();
                Ok(config)
            })?;
            Ok(Response::new()
                .add_attributes(vec![
                    ("alliance_token_denom", denom.clone()),
                    ("alliance_token_total_supply", total_supply.to_string()),
                ])
                .add_submessage(sub_msg))
        }
        _ => Err(ContractError::InvalidReplyId(reply.id)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    #[test]
    fn proper_initialization() {}
}
