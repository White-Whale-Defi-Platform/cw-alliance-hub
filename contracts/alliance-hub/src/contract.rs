use std::cmp::min;
use std::collections::{HashMap, HashSet};

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    ensure, from_json, to_json_binary, Addr, Binary, Coin as CwCoin, CosmosMsg, Decimal, DepsMut,
    Empty, Env, MessageInfo, Order, Reply, Response, StdError, StdResult, Storage, SubMsg,
    Timestamp, Uint128, WasmMsg,
};
use cw2::{get_contract_version, set_contract_version};
use cw20::Cw20ReceiveMsg;
use cw_asset::{Asset, AssetInfo, AssetInfoBase};
use cw_utils::parse_instantiate_response_data;
use semver::Version;
use terra_proto_rs::alliance::alliance::{
    MsgClaimDelegationRewards, MsgDelegate, MsgRedelegate, MsgUndelegate,
};
use terra_proto_rs::cosmos::base::v1beta1::Coin;
use terra_proto_rs::traits::Message;
use ve3_shared::constants::SECONDS_PER_YEAR;
use ve3_shared::error::SharedError;
use ve3_shared::extensions::asset_info_ext::AssetInfoExt;
use ve3_shared::msgs_asset_staking::AssetConfigRuntime;
use ve3_shared::stake_config::StakeConfig;

// use alliance_protocol::alliance_oracle_types::QueryMsg as OracleQueryMsg;
use alliance_protocol::alliance_protocol::{
    AllianceDelegateMsg, AllianceRedelegateMsg, AllianceUndelegateMsg, AssetDistribution,
    AssetInfoWithConfig, ChainId, Config, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg,
};

// use alliance_protocol::alliance_oracle_types::{AssetStaked, ChainId, EmissionsDistribution};
use crate::error::ContractError;
use crate::migrations::migrate_state;
use crate::state::{
    ASSET_CONFIG, ASSET_REWARD_DISTRIBUTION, ASSET_REWARD_RATE, CONFIG, SHARES, TEMP_BALANCE,
    TOTAL_BALANCES_SHARES, UNCLAIMED_REWARDS, USER_ASSET_REWARD_RATE, VALIDATORS, WHITELIST,
};
use crate::token_factory::{CustomExecuteMsg, DenomUnit, Metadata, TokenExecuteMsg};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:terra-alliance-protocol";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const CREATE_REPLY_ID: u64 = 1;
const CLAIM_REWARD_ERROR_REPLY_ID: u64 = 2;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(mut deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let version: Version = CONTRACT_VERSION.parse()?;
    let storage_version: Version = get_contract_version(deps.storage)?.version.parse()?;

    ensure!(
        storage_version < version,
        StdError::generic_err("Invalid contract version")
    );

    migrate_state(deps.branch())?;

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response<CustomExecuteMsg>, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let governance_address = deps.api.addr_validate(msg.governance.as_str())?;
    let controller_address = deps.api.addr_validate(msg.controller.as_str())?;
    let oracle_address = deps.api.addr_validate(msg.oracle.as_str())?;
    let operator_address = deps.api.addr_validate(msg.operator.as_str())?;
    let take_rate_taker_address = deps.api.addr_validate(msg.take_rate_taker.as_str())?;
    let create_msg = TokenExecuteMsg::CreateDenom {
        subdenom: msg.alliance_token_denom.to_string(),
    };
    let sub_msg = SubMsg::reply_on_success(
        CosmosMsg::Custom(CustomExecuteMsg::Token(create_msg)),
        CREATE_REPLY_ID,
    );

    // We set asset_reward_distribution here or manually via an execute method otherwise there is no distribution ratio
    // asset_reward_distribution is a list of AssetDistribution which is a struct that contains an AssetInfo and a Decimal.
    // ASSET_REWARD_DISTRIBUTION.save(deps.storage, &vec![
    //         AssetDistribution {
    //             asset: AssetInfo::Native("uluna".to_string()),
    //             distribution: Decimal::percent(50),
    //         },
    //         AssetDistribution {
    //             asset: AssetInfo::Native("usdr".to_string()),
    //             distribution: Decimal::percent(50),
    //         },
    //     ])?;

    let config = Config {
        governance: governance_address,
        controller: controller_address,
        oracle: oracle_address,
        operator: operator_address,
        take_rate_taker: take_rate_taker_address,
        alliance_token_denom: "".to_string(),
        alliance_token_supply: Uint128::zero(),
        last_reward_update_timestamp: Timestamp::default(),
        reward_denom: msg.reward_denom,
        default_yearly_take_rate: msg.default_yearly_take_rate,
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
        // Enable support for staking and unstaking of Cw20Assets
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::WhitelistAssets(assets) => whitelist_assets(deps, info, assets),
        ExecuteMsg::RemoveAssets(assets) => remove_assets(deps, info, assets),
        ExecuteMsg::Stake {} => {
            if info.funds.len() != 1 {
                return Err(ContractError::OnlySingleAssetAllowed {});
            }
            if info.funds[0].amount.is_zero() {
                return Err(ContractError::AmountCannotBeZero {});
            }
            let asset = AssetInfo::native(&info.funds[0].denom);
            stake(
                deps,
                env,
                info.clone(),
                asset,
                info.funds[0].amount,
                info.sender,
            )
        }
        ExecuteMsg::Unstake(asset) => unstake(deps, env, info, asset),
        ExecuteMsg::ClaimRewards(asset) => claim_rewards(deps, info, asset),
        ExecuteMsg::UpdateRewards {} => update_rewards(deps, env, info),
        // ualliance token delegation methods
        ExecuteMsg::AllianceDelegate(msg) => alliance_delegate(deps, env, info, msg),
        ExecuteMsg::AllianceUndelegate(msg) => alliance_undelegate(deps, env, info, msg),
        ExecuteMsg::AllianceRedelegate(msg) => alliance_redelegate(deps, env, info, msg),
        ExecuteMsg::UpdateRewardsCallback {} => update_reward_callback(deps, env, info),
        ExecuteMsg::SetAssetRewardDistribution(asset_reward_distribution) => {
            set_asset_reward_distribution(deps, info, asset_reward_distribution)
        }
        // The below two ExecuteMsg are disabled with this variant. Instead of rebalancing emissions based on staking, it is manually configured through governance and can be reconfigured through the same method
        // ExecuteMsg::RebalanceEmissions {} => rebalance_emissions(deps, env, info),
        // ExecuteMsg::RebalanceEmissionsCallback {} => rebalance_emissions_callback(deps, env, info),
        // Allow Governance to overwrite the AssetDistributions for the reward emissions
        // Generic unsupported handler returns a StdError
        ExecuteMsg::DistributeTakeRate { update, assets } => {
            distribute_take_rate(deps, env, info, update, assets)
        }
        ExecuteMsg::UpdateAssetConfig(asset_config) => {
            update_asset_config(deps, env, info, asset_config)
        }
        ExecuteMsg::UpdateConfig {
            governance,
            controller,
            oracle,
            operator,
            take_rate_taker,
            default_yearly_take_rate,
        } => update_config(
            deps,
            info,
            governance,
            controller,
            oracle,
            operator,
            take_rate_taker,
            default_yearly_take_rate,
        ),
        _ => Err(ContractError::Std(StdError::generic_err(
            "unsupported action",
        ))),
    }
}

// receive_cw20 routes a cw20 token to the proper handler in this case stake and unstake
fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let sender = deps.api.addr_validate(&cw20_msg.sender)?;

    match from_json(&cw20_msg.msg)? {
        Cw20HookMsg::Stake {} => {
            if cw20_msg.amount.is_zero() {
                return Err(ContractError::AmountCannotBeZero {});
            }
            let asset = AssetInfo::Cw20(info.sender.clone());
            stake(deps, env, info, asset, cw20_msg.amount, sender)
        }
        Cw20HookMsg::Unstake(asset) => unstake(deps, env, info, asset),
    }
}

fn set_asset_reward_distribution(
    deps: DepsMut,
    info: MessageInfo,
    asset_reward_distribution: Vec<AssetDistribution>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    is_authorized(&info, &config)?;

    // Ensure the dsitributions add up to 100%
    let total_distribution = asset_reward_distribution
        .iter()
        .map(|a| a.distribution)
        .fold(Decimal::zero(), |acc, v| acc + v);

    if total_distribution != Decimal::percent(100) {
        return Err(ContractError::InvalidDistribution {});
    }

    // Simply set the asset_reward_distribution, overwriting any previous settings.
    // This means any updates should include the full existing set of AssetDistributions and not just the newly updated one.
    ASSET_REWARD_DISTRIBUTION.save(deps.storage, &asset_reward_distribution)?;
    Ok(Response::new().add_attributes(vec![("action", "set_asset_reward_distribution")]))
}

fn whitelist_assets(
    deps: DepsMut,
    info: MessageInfo,
    assets_request: HashMap<ChainId, Vec<AssetInfoWithConfig>>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    is_governance(&info, &config)?;

    let mut attrs = vec![("action".to_string(), "whitelist_assets".to_string())];

    for (chain_id, assets) in &assets_request {
        for asset_with_config in assets {
            WHITELIST.save(deps.storage, &asset_with_config.info, chain_id)?;
            ASSET_REWARD_RATE.update(
                deps.storage,
                &asset_with_config.info,
                |rate| -> StdResult<_> { Ok(rate.unwrap_or(Decimal::zero())) },
            )?;

            let current_config = ASSET_CONFIG
                .may_load(deps.storage, &asset_with_config.info)?
                .unwrap_or_default();

            let new_yearly_take_rate = asset_with_config
                .yearly_take_rate
                .unwrap_or(current_config.yearly_take_rate);

            ASSET_CONFIG.save(
                deps.storage,
                &asset_with_config.info,
                &AssetConfigRuntime {
                    yearly_take_rate: new_yearly_take_rate,
                    stake_config: StakeConfig::Default, // dummy value

                    last_taken_s: 0,
                    taken: current_config.taken,
                    harvested: current_config.harvested,
                },
            )?;
        }

        let assets_str = assets
            .iter()
            .map(|asset| asset.info.to_string())
            .collect::<Vec<String>>()
            .join(",");

        attrs.push(("chain_id".to_string(), chain_id.to_string()));
        attrs.push(("assets".to_string(), assets_str.to_string()));
    }
    Ok(Response::new().add_attributes(attrs))
}

fn remove_assets(
    deps: DepsMut,
    info: MessageInfo,
    assets: Vec<AssetInfo>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    // Only allow the governance address to update whitelisted assets
    is_governance(&info, &config)?;
    for asset in &assets {
        WHITELIST.remove(deps.storage, asset);
    }
    let assets_str = assets
        .iter()
        .map(|asset| asset.to_string())
        .collect::<Vec<String>>()
        .join(",");
    Ok(Response::new().add_attributes(vec![("action", "remove_assets"), ("assets", &assets_str)]))
}

fn stake(
    mut deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    asset: AssetInfoBase<Addr>,
    amount: Uint128,
    recipient: Addr,
) -> Result<Response, ContractError> {
    assert_asset_whitelisted(&deps, &asset)?;

    let rewards = _claim_reward(deps.storage, recipient.clone(), asset.clone())?;
    if !rewards.is_zero() {
        UNCLAIMED_REWARDS.update(
            deps.storage,
            (recipient.clone(), &asset),
            |balance| -> Result<_, ContractError> {
                Ok(balance.unwrap_or(Uint128::zero()) + rewards)
            },
        )?;
    }

    let (balance, shares) = TOTAL_BALANCES_SHARES
        .may_load(deps.storage, &asset)?
        .unwrap_or_default();
    let (_, asset_available) = _take(&mut deps, &env, &asset, balance, true)?;
    let share_amount = compute_share_amount(shares, amount, asset_available);

    SHARES.update(
        deps.storage,
        (recipient.clone(), &asset),
        |balance| -> Result<_, ContractError> {
            Ok(balance.unwrap_or_default().checked_add(share_amount)?)
        },
    )?;

    TOTAL_BALANCES_SHARES.save(
        deps.storage,
        &asset,
        &(
            balance.checked_add(amount)?,
            shares.checked_add(share_amount)?,
        ),
    )?;

    let asset_reward_rate = ASSET_REWARD_RATE
        .load(deps.storage, &asset)
        .unwrap_or(Decimal::zero());
    USER_ASSET_REWARD_RATE.save(
        deps.storage,
        (recipient.clone(), &asset),
        &asset_reward_rate,
    )?;

    Ok(Response::new().add_attributes(vec![
        ("action", "stake"),
        ("user", recipient.as_ref()),
        ("asset", &asset.to_string()),
        ("amount", &amount.to_string()),
        ("share", &share_amount.to_string()),
    ]))
}

fn unstake(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    asset: Asset,
) -> Result<Response, ContractError> {
    let sender = info.sender.clone();
    ensure!(
        !asset.amount.is_zero(),
        ContractError::AmountCannotBeZero {}
    );

    let rewards = _claim_reward(deps.storage, sender.clone(), asset.info.clone())?;
    if !rewards.is_zero() {
        UNCLAIMED_REWARDS.update(
            deps.storage,
            (sender.clone(), &asset.info),
            |balance| -> Result<_, ContractError> {
                Ok(balance.unwrap_or(Uint128::zero()) + rewards)
            },
        )?;
    }

    let (balance, shares) = TOTAL_BALANCES_SHARES
        .may_load(deps.storage, &asset.info)?
        .unwrap_or_default();
    let (_, asset_available) = _take(&mut deps, &env, &asset.info, balance, true)?;

    let mut withdraw_amount = asset.amount;
    let mut share_amount = compute_share_amount(shares, withdraw_amount, asset_available);

    let current_user_share = SHARES
        .may_load(deps.storage, (sender.clone(), &asset.info))?
        .unwrap_or_default();

    ensure!(
        !current_user_share.is_zero(),
        ContractError::AmountCannotBeZero {}
    );

    if current_user_share < share_amount {
        share_amount = current_user_share;
        withdraw_amount = compute_balance_amount(shares, share_amount, asset_available)
    }

    SHARES.save(
        deps.storage,
        (sender, &asset.info),
        &(current_user_share.checked_sub(share_amount)?),
    )?;

    TOTAL_BALANCES_SHARES.save(
        deps.storage,
        &asset.info,
        &(
            balance
                .checked_sub(withdraw_amount)
                .map_err(|_| SharedError::InsufficientBalance("total balance".to_string()))?,
            shares
                .checked_sub(share_amount)
                .map_err(|_| SharedError::InsufficientBalance("total shares".to_string()))?,
        ),
    )?;

    let msg = asset
        .info
        .with_balance(withdraw_amount)
        .transfer_msg(&info.sender)?;

    Ok(Response::new()
        .add_attributes(vec![
            ("action", "unstake"),
            ("user", info.sender.as_ref()),
            ("asset", &asset.info.to_string()),
            ("amount", &withdraw_amount.to_string()),
        ])
        .add_message(msg))
}

fn claim_rewards(
    deps: DepsMut,
    info: MessageInfo,
    asset_info: AssetInfo,
) -> Result<Response, ContractError> {
    let user = info.sender;
    let config = CONFIG.load(deps.storage)?;
    let rewards = _claim_reward(deps.storage, user.clone(), asset_info.clone())?;
    let unclaimed_rewards = UNCLAIMED_REWARDS
        .load(deps.storage, (user.clone(), &asset_info))
        .unwrap_or(Uint128::zero());
    let final_rewards = rewards + unclaimed_rewards;
    UNCLAIMED_REWARDS.remove(deps.storage, (user.clone(), &asset_info));
    let response = Response::new().add_attributes(vec![
        ("action", "claim_rewards"),
        ("user", user.as_ref()),
        ("asset", &asset_info.to_string()),
        ("reward_amount", &final_rewards.to_string()),
    ]);
    if !final_rewards.is_zero() {
        let rewards_asset = Asset {
            info: AssetInfo::Native(config.reward_denom),
            amount: final_rewards,
        };
        Ok(response.add_message(rewards_asset.transfer_msg(&user)?))
    } else {
        Ok(response)
    }
}

fn _claim_reward(
    storage: &mut dyn Storage,
    user: Addr,
    asset_info: AssetInfo,
) -> Result<Uint128, ContractError> {
    let user_reward_rate = USER_ASSET_REWARD_RATE.load(storage, (user.clone(), &asset_info));
    let asset_reward_rate = ASSET_REWARD_RATE.load(storage, &asset_info)?;

    if let Ok(user_reward_rate) = user_reward_rate {
        let user_staked = SHARES.load(storage, (user.clone(), &asset_info))?;
        let rewards = ((asset_reward_rate - user_reward_rate)
            * Decimal::from_atomics(user_staked, 0)?)
        .to_uint_floor();
        if rewards.is_zero() {
            Ok(Uint128::zero())
        } else {
            USER_ASSET_REWARD_RATE.save(storage, (user, &asset_info), &asset_reward_rate)?;
            Ok(rewards)
        }
    } else {
        // If cannot find user_reward_rate, assume this is the first time they are staking and set it to the current asset_reward_rate
        USER_ASSET_REWARD_RATE.save(storage, (user, &asset_info), &asset_reward_rate)?;

        Ok(Uint128::zero())
    }
}

fn alliance_delegate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: AllianceDelegateMsg,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    is_controller(&info, &config)?;
    if msg.delegations.is_empty() {
        return Err(ContractError::EmptyDelegation {});
    }
    let mut validators = VALIDATORS.load(deps.storage)?;
    let mut msgs: Vec<CosmosMsg<Empty>> = vec![];
    for delegation in msg.delegations {
        let delegate_msg = MsgDelegate {
            amount: Some(Coin {
                denom: config.alliance_token_denom.clone(),
                amount: delegation.amount.to_string(),
            }),
            delegator_address: env.contract.address.to_string(),
            validator_address: delegation.validator.to_string(),
        };
        msgs.push(CosmosMsg::Stargate {
            type_url: "/alliance.alliance.MsgDelegate".to_string(),
            value: Binary::from(delegate_msg.encode_to_vec()),
        });
        validators.insert(delegation.validator);
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
    is_controller(&info, &config)?;
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
    is_controller(&info, &config)?;
    if msg.redelegations.is_empty() {
        return Err(ContractError::EmptyDelegation {});
    }
    let mut msgs = vec![];
    let mut validators = VALIDATORS.load(deps.storage)?;
    for redelegation in msg.redelegations {
        let src_validator = redelegation.src_validator;
        let dst_validator = redelegation.dst_validator;
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
    // since contract balance = previous contract balance + sent balance > sent balance
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
                type_url: "/alliance.alliance.MsgClaimDelegationRewards".to_string(),
                value: Binary::from(msg.encode_to_vec()),
            };
            // Reply on error here is used to ignore errors from claiming rewards with validators that we did not delegate to
            SubMsg::reply_on_error(msg, CLAIM_REWARD_ERROR_REPLY_ID)
        })
        .collect();
    let msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.to_string(),
        msg: to_json_binary(&ExecuteMsg::UpdateRewardsCallback {}).unwrap(),
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
    ensure!(
        info.sender == env.contract.address,
        ContractError::Unauthorized {}
    );

    let config = CONFIG.load(deps.storage)?;
    let reward_asset = AssetInfo::native(config.reward_denom);
    let current_balance = reward_asset.query_balance(&deps.querier, env.contract.address)?;
    let previous_balance = TEMP_BALANCE.load(deps.storage)?;
    let rewards_collected = current_balance - previous_balance;

    let asset_reward_distribution = ASSET_REWARD_DISTRIBUTION.load(deps.storage)?;
    let total_distribution = asset_reward_distribution
        .iter()
        .map(|a| a.distribution)
        .fold(Decimal::zero(), |acc, v| acc + v);

    for asset_distribution in asset_reward_distribution {
        let total_reward_distributed = Decimal::from_atomics(rewards_collected, 0)?
            * asset_distribution.distribution
            / total_distribution;

        // If there are no balances, we stop updating the rate. This means that the emissions are not directed to any stakers.
        let (_, total_shares) = TOTAL_BALANCES_SHARES
            .load(deps.storage, &asset_distribution.asset)
            .unwrap_or_default();
        if !total_shares.is_zero() {
            let rate_to_update = total_reward_distributed / Decimal::from_atomics(total_shares, 0)?;
            if rate_to_update > Decimal::zero() {
                ASSET_REWARD_RATE.update(
                    deps.storage,
                    &asset_distribution.asset,
                    |rate| -> StdResult<_> { Ok(rate.unwrap_or(Decimal::zero()) + rate_to_update) },
                )?;
            }
        }
    }
    TEMP_BALANCE.remove(deps.storage);

    Ok(Response::new().add_attributes(vec![("action", "update_rewards_callback")]))
}

#[allow(clippy::too_many_arguments)]
fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    governance: Option<String>,
    controller: Option<String>,
    oracle: Option<String>,
    operator: Option<String>,
    take_rate_taker: Option<String>,
    default_yearly_take_rate: Option<Decimal>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    ensure!(
        info.sender == config.governance,
        ContractError::Unauthorized {}
    );

    if let Some(governance) = governance {
        config.governance = deps.api.addr_validate(&governance)?;
    }

    if let Some(controller) = controller {
        config.controller = deps.api.addr_validate(&controller)?;
    }

    if let Some(oracle) = oracle {
        config.oracle = deps.api.addr_validate(&oracle)?;
    }

    if let Some(operator) = operator {
        config.operator = deps.api.addr_validate(&operator)?;
    }

    if let Some(take_rate_taker) = take_rate_taker {
        config.take_rate_taker = deps.api.addr_validate(&take_rate_taker)?;
    }

    if let Some(default_yearly_take_rate) = default_yearly_take_rate {
        config.default_yearly_take_rate = default_yearly_take_rate;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attributes(vec![("action", "update_config")]))
}

// fn rebalance_emissions(
//     deps: DepsMut,
//     env: Env,
//     info: MessageInfo,
// ) -> Result<Response, ContractError> {
//     // Allow execution only from the controller account
//     let config = CONFIG.load(deps.storage)?;
//     is_controller(&info, &config)?;
//     // Before starting with the rebalance emission process
//     // rewards must be updated to the current block height
//     // Skip if no reward distribution in the first place
//     let res = if ASSET_REWARD_DISTRIBUTION.load(deps.storage).is_ok() {
//         update_rewards(deps, env.clone(), info)?
//     } else {
//         Response::new()
//     };

//     Ok(res.add_message(CosmosMsg::Wasm(WasmMsg::Execute {
//         contract_addr: env.contract.address.to_string(),
//         msg: to_json_binary(&ExecuteMsg::RebalanceEmissionsCallback {}).unwrap(),
//         funds: vec![],
//     })))
// }

// fn rebalance_emissions_callback(
//     deps: DepsMut,
//     env: Env,
//     info: MessageInfo,
// ) -> Result<Response, ContractError> {
//     if info.sender != env.contract.address {
//         return Err(ContractError::Unauthorized {});
//     }
//     let config = CONFIG.load(deps.storage)?;

//     // This is the request that will be send to the oracle contract
//     // on the QueryEmissionsDistributions entry point to recover
//     // the assets_reward_distribution...
//     let mut distr_req: HashMap<ChainId, Vec<AssetStaked>> = HashMap::new();

//     let whitelist: Vec<(AssetInfoUnchecked, ChainId)> = WHITELIST
//         .range(deps.storage, None, None, Order::Ascending)
//         .map(|item| item.unwrap())
//         .collect();
//     for (asset, chain_id) in whitelist {
//         let asset = asset.check(deps.api, None)?;
//         let total_balance = TOTAL_BALANCES
//             .load(deps.storage, AssetInfoKey::from(asset.clone()))
//             .unwrap_or(Uint128::zero());

//         // Oracle does not support non-native coins so skip if non-native
//         if let AssetInfoBase::Native(denom) = asset {
//             distr_req
//                 .entry(chain_id)
//                 .or_insert_with(Vec::new)
//                 .push(AssetStaked {
//                     denom,
//                     amount: total_balance,
//                 });
//         }
//     }

//     // Query oracle contract for the new distribution
//     let distr_res: Vec<EmissionsDistribution> = deps.querier.query_wasm_smart(
//         config.oracle,
//         &OracleQueryMsg::QueryEmissionsDistributions(distr_req),
//     )?;

//     let asset_reward_distribution: StdResult<Vec<AssetDistribution>> = distr_res
//         .iter()
//         .map(|d| -> StdResult<AssetDistribution> {
//             let distribution = d.distribution.to_decimal()?;
//             Ok(AssetDistribution {
//                 asset: AssetInfo::Native(d.denom.to_string()),
//                 distribution,
//             })
//         })
//         .collect();
//     let asset_reward_distribution = asset_reward_distribution?;
//     ASSET_REWARD_DISTRIBUTION.save(deps.storage, &asset_reward_distribution)?;

//     let mut attrs = vec![("action".to_string(), "rebalance_emissions".to_string())];
//     for distribution in asset_reward_distribution {
//         attrs.push((
//             distribution.asset.to_string(),
//             distribution.distribution.to_string(),
//         ));
//     }
//     Ok(Response::new().add_attributes(attrs))
// }

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
            let total_supply = Uint128::from(1_000_000_000_000_u128);
            let sub_msg_mint = SubMsg::new(CosmosMsg::Custom(CustomExecuteMsg::Token(
                TokenExecuteMsg::MintTokens {
                    denom: denom.clone(),
                    amount: total_supply,
                    mint_to_address: env.contract.address.to_string(),
                },
            )));
            CONFIG.update(deps.storage, |mut config| -> Result<_, ContractError> {
                config.alliance_token_denom = denom.clone();
                config.alliance_token_supply = total_supply;
                Ok(config)
            })?;
            let symbol = "ALLIANCE";

            let sub_msg_metadata = SubMsg::new(CosmosMsg::Custom(CustomExecuteMsg::Token(
                TokenExecuteMsg::SetMetadata {
                    denom: denom.clone(),
                    metadata: Metadata {
                        description: "Staking token for the alliance protocol".to_string(),
                        denom_units: vec![DenomUnit {
                            denom: denom.clone(),
                            exponent: 0,
                            aliases: vec![],
                        }],
                        base: denom.to_string(),
                        display: denom.to_string(),
                        name: "Alliance Token".to_string(),
                        symbol: symbol.to_string(),
                    },
                },
            )));
            Ok(Response::new()
                .add_attributes(vec![
                    ("alliance_token_denom", denom),
                    ("alliance_token_total_supply", total_supply.to_string()),
                ])
                .add_submessage(sub_msg_mint)
                .add_submessage(sub_msg_metadata))
        }
        CLAIM_REWARD_ERROR_REPLY_ID => {
            Ok(Response::new().add_attributes(vec![("action", "claim_reward_error")]))
        }
        _ => Err(ContractError::InvalidReplyId(reply.id)),
    }
}

fn distribute_take_rate(
    mut deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    update: Option<bool>,
    assets: Option<Vec<AssetInfo>>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let assets = if let Some(assets) = assets {
        assets
    } else {
        WHITELIST
            .keys(deps.storage, None, None, Order::Ascending)
            .collect::<StdResult<_>>()?
    };

    let mut response = Response::new().add_attributes(vec![("action", "distribute_take_rate")]);
    let recipient = config.take_rate_taker;
    for asset in assets {
        let mut config = if update == Some(true) {
            // if it should also update extraction, take the asset config from the result.
            let (balance, _) = TOTAL_BALANCES_SHARES
                .may_load(deps.storage, &asset)?
                .unwrap_or_default();
            // no need to save, as we will save it anyways
            let (config, _) = _take(&mut deps, &env, &asset, balance, false)?;
            config
        } else {
            // otherwise just load it.
            ASSET_CONFIG
                .may_load(deps.storage, &asset)?
                .unwrap_or_default()
        };

        let take_amount = config.taken.checked_sub(config.harvested)?;
        if take_amount.is_zero() {
            response = response.add_attribute(asset.to_string(), "skip");
            continue;
        }

        let take_asset = asset.with_balance(take_amount);

        config.harvested = config.taken;
        ASSET_CONFIG.save(deps.storage, &asset, &config)?;

        // unstake assets if necessary
        let unstake_msgs =
            config
                .stake_config
                .unstake_check_received_msg(&deps, &env, take_asset.clone())?;
        // transfer to recipient
        let take_msg = take_asset.transfer_msg(recipient.clone())?;

        response = response
            .add_messages(unstake_msgs)
            .add_message(take_msg)
            .add_attribute("take", format!("{0}{1}", take_amount, asset));
    }
    Ok(response)
}

fn _take(
    deps: &mut DepsMut,
    env: &Env,
    asset: &AssetInfo,
    total_balance: Uint128,
    save_config: bool,
) -> Result<(AssetConfigRuntime, Uint128), ContractError> {
    let config = ASSET_CONFIG.may_load(deps.storage, asset)?;

    if let Some(mut config) = config {
        if config.yearly_take_rate.is_zero() {
            let available = total_balance.checked_sub(config.taken)?;
            return Ok((config, available));
        }

        // only take if last taken set
        if config.last_taken_s != 0 {
            let take_diff_s = Uint128::new((env.block.time.seconds() - config.last_taken_s).into());
            let relevant_balance = total_balance.saturating_sub(config.taken);
            let take_amount = config.yearly_take_rate
                * relevant_balance.multiply_ratio(
                    min(take_diff_s, Uint128::new(SECONDS_PER_YEAR.into())),
                    SECONDS_PER_YEAR,
                );

            config.taken = config.taken.checked_add(take_amount)?;
        }

        config.last_taken_s = env.block.time.seconds();

        if save_config {
            ASSET_CONFIG.save(deps.storage, asset, &config)?;
        }

        let available = total_balance.checked_sub(config.taken)?;
        return Ok((config, available));
    }

    Ok((AssetConfigRuntime::default(), total_balance))
}

fn update_asset_config(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    update: AssetInfoWithConfig,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    is_governance(&info, &config)?;
    assert_asset_whitelisted(&deps, &update.info)?;

    let current = ASSET_CONFIG
        .may_load(deps.storage, &update.info)?
        .unwrap_or_default();

    let mut updated = current.clone();

    let new_yearly_take_rate = update
        .yearly_take_rate
        .unwrap_or(config.default_yearly_take_rate);

    updated.yearly_take_rate = new_yearly_take_rate;
    // dummy value
    updated.stake_config = ve3_shared::stake_config::StakeConfig::Default;
    ASSET_CONFIG.save(deps.storage, &update.info, &updated)?;

    let mut msgs = vec![];
    if current.stake_config != updated.stake_config {
        // if stake config changed, withdraw from one (or do nothing), deposit on the other.
        let (balance, _) = TOTAL_BALANCES_SHARES.load(deps.storage, &update.info)?;
        let available = balance - current.taken;
        let asset = update.info.with_balance(available);

        let mut unstake_msgs =
            current
                .stake_config
                .unstake_check_received_msg(&deps, &env, asset.clone())?;
        let mut stake_msgs = updated
            .stake_config
            .stake_check_received_msg(&deps, &env, asset)?;

        msgs.append(&mut unstake_msgs);
        msgs.append(&mut stake_msgs);
    }

    Ok(Response::new().add_attributes(vec![
        ("action", "update_asset_config"),
        ("asset", &update.info.to_string()),
    ]))
}

pub(crate) fn compute_share_amount(
    shares: Uint128,
    balance_amount: Uint128,
    asset_available: Uint128,
) -> Uint128 {
    if asset_available.is_zero() {
        balance_amount
    } else if shares == asset_available {
        return balance_amount;
    } else {
        balance_amount.multiply_ratio(shares, asset_available)
    }
}

pub(crate) fn compute_balance_amount(
    shares: Uint128,
    share_amount: Uint128,
    asset_available: Uint128,
) -> Uint128 {
    if shares.is_zero() {
        Uint128::zero()
    } else if shares == asset_available {
        return share_amount;
    } else {
        share_amount.multiply_ratio(asset_available, shares)
    }
}

// Controller is used to perform administrative operations that deals with delegating the virtual
// tokens to the expected validators
fn is_controller(info: &MessageInfo, config: &Config) -> Result<(), ContractError> {
    ensure!(
        info.sender == config.controller,
        ContractError::Unauthorized {}
    );

    Ok(())
}

// Only governance (through a on-chain prop) can change the whitelisted assets
fn is_governance(info: &MessageInfo, config: &Config) -> Result<(), ContractError> {
    ensure!(
        info.sender == config.governance,
        ContractError::Unauthorized {}
    );

    Ok(())
}

// Only governance or the operator can pass through this function
fn is_authorized(info: &MessageInfo, config: &Config) -> Result<(), ContractError> {
    ensure!(
        info.sender == config.governance || info.sender == config.operator,
        ContractError::Unauthorized {}
    );
    Ok(())
}

fn assert_asset_whitelisted(deps: &DepsMut, asset: &AssetInfo) -> Result<ChainId, ContractError> {
    WHITELIST
        .load(deps.storage, asset)
        .map_err(|_| ContractError::AssetNotWhitelisted {})
}
