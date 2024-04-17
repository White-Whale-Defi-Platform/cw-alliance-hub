use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cosmwasm_std::{
    from_binary, Addr, Binary, CosmosMsg, Reply, Response, SubMsg, SubMsgResponse, SubMsgResult,
    Timestamp, Uint128,
};
use terra_proto_rs::traits::MessageExt;

use alliance_protocol::alliance_protocol::ExecuteMsg::UpdateConfig;
use alliance_protocol::alliance_protocol::{Config, QueryMsg};

use crate::contract::{execute, reply};
use crate::query::query;
use crate::tests::helpers::setup_contract;
use crate::token_factory::{CustomExecuteMsg, DenomUnit, Metadata, TokenExecuteMsg};

#[test]
fn test_setup_contract() {
    let mut deps = mock_dependencies();
    let res = setup_contract(deps.as_mut());
    let denom = "ualliance";
    assert_eq!(
        res,
        Response::default()
            .add_attributes(vec![("action", "instantiate")])
            .add_submessage(SubMsg::reply_on_success(
                CosmosMsg::Custom(CustomExecuteMsg::Token(TokenExecuteMsg::CreateDenom {
                    subdenom: denom.to_string(),
                })),
                1,
            ))
    );

    // Instantiate is a two steps process that's why
    // alliance_token_denom and alliance_token_supply
    // will be populated on reply.
    let query_config = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: Config = from_binary(&query_config).unwrap();
    assert_eq!(
        config,
        Config {
            governance: Addr::unchecked("gov"),
            controller: Addr::unchecked("controller"),
            oracle: Addr::unchecked("oracle"),
            operator: Addr::unchecked("operator"),
            reward_denom: "uluna".to_string(),
            alliance_token_denom: "".to_string(),
            alliance_token_supply: Uint128::new(0),
            last_reward_update_timestamp: Timestamp::default(),
        }
    );
}

#[test]
fn test_reply_create_token() {
    let mut deps = mock_dependencies();
    setup_contract(deps.as_mut());

    // Build reply message
    let msg = Reply {
        id: 1,
        result: SubMsgResult::Ok(SubMsgResponse {
            events: vec![],
            data: Some(Binary::from(
                String::from("factory/cosmos2contract/ualliance")
                    .to_bytes()
                    .unwrap(),
            )),
        }),
    };
    let res = reply(deps.as_mut(), mock_env(), msg).unwrap();
    let sub_msg = SubMsg::new(CosmosMsg::Custom(CustomExecuteMsg::Token(
        TokenExecuteMsg::MintTokens {
            amount: Uint128::from(1000000000000u128),
            denom: "factory/cosmos2contract/ualliance".to_string(),
            mint_to_address: "cosmos2contract".to_string(),
        },
    )));
    let sub_msg_metadata = SubMsg::new(CosmosMsg::Custom(CustomExecuteMsg::Token(
        TokenExecuteMsg::SetMetadata {
            denom: "factory/cosmos2contract/ualliance".to_string(),
            metadata: Metadata {
                description: "Staking token for the alliance protocol".to_string(),
                denom_units: vec![DenomUnit {
                    denom: "factory/cosmos2contract/ualliance".to_string(),
                    exponent: 0,
                    aliases: vec![],
                }],
                base: "factory/cosmos2contract/ualliance".to_string(),
                display: "factory/cosmos2contract/ualliance".to_string(),
                name: "Alliance Token".to_string(),
                symbol: "ALLIANCE".to_string(),
            },
        },
    )));
    assert_eq!(
        res,
        Response::default()
            .add_attributes(vec![
                ("alliance_token_denom", "factory/cosmos2contract/ualliance"),
                ("alliance_token_total_supply", "1000000000000"),
            ])
            .add_submessage(sub_msg)
            .add_submessage(sub_msg_metadata)
    );

    let query_config = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: Config = from_binary(&query_config).unwrap();
    assert_eq!(
        config,
        Config {
            governance: Addr::unchecked("gov"),
            controller: Addr::unchecked("controller"),
            oracle: Addr::unchecked("oracle"),
            operator: Addr::unchecked("operator"),
            reward_denom: "uluna".to_string(),
            alliance_token_denom: "factory/cosmos2contract/ualliance".to_string(),
            alliance_token_supply: Uint128::new(1000000000000),
            last_reward_update_timestamp: Timestamp::default(),
        }
    );
}

#[test]
fn test_update_config() {
    let mut deps = mock_dependencies();
    setup_contract(deps.as_mut());

    let query_config = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let Config {
        governance,
        controller,
        oracle,
        operator,
        ..
    } = from_binary(&query_config).unwrap();

    assert_eq!(governance, Addr::unchecked("gov"));
    assert_eq!(controller, Addr::unchecked("controller"));
    assert_eq!(oracle, Addr::unchecked("oracle"));
    assert_eq!(operator, Addr::unchecked("operator"));

    let msg = UpdateConfig {
        governance: Some("new_gov".to_string()),
        controller: Some("new_controller".to_string()),
        oracle: Some("new_oracle".to_string()),
        operator: Some("new_operator".to_string()),
    };

    let result = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("unauthorized", &[]),
        msg.clone(),
    );

    if result.is_ok() {
        panic!("should be unauthorized")
    }

    let result = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("operator", &[]),
        msg.clone(),
    );

    if result.is_ok() {
        panic!("should be unauthorized")
    }

    let result = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("controller", &[]),
        msg.clone(),
    );

    if result.is_ok() {
        panic!("should be unauthorized")
    }

    let result = execute(deps.as_mut(), mock_env(), mock_info("gov", &[]), msg);

    if result.is_err() {
        panic!("should be fine")
    }

    let query_config = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let Config {
        governance,
        controller,
        oracle,
        operator,
        ..
    } = from_binary(&query_config).unwrap();

    assert_eq!(governance, Addr::unchecked("new_gov"));
    assert_eq!(controller, Addr::unchecked("new_controller"));
    assert_eq!(oracle, Addr::unchecked("new_oracle"));
    assert_eq!(operator, Addr::unchecked("new_operator"));
}
