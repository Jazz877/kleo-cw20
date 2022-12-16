use cosmwasm_std::{testing::{mock_dependencies, mock_env}, to_binary, Addr, CosmosMsg, Decimal, Empty, Uint128, WasmMsg, Timestamp};
use cw2::ContractVersion;
use cw20::{BalanceResponse, Cw20Coin, MinterResponse, TokenInfoResponse};
use cw_core_interface::voting::{InfoResponse, IsActiveResponse, TotalPowerAtHeightResponse, VotingPowerAtHeightResponse};
use cw_multi_test::{next_block, App, Contract, ContractWrapper, Executor};
use cw_utils::{Duration};

use crate::{
    contract::{migrate, CONTRACT_NAME, CONTRACT_VERSION},
    msg::{
        ActiveThreshold, ActiveThresholdResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
        StakingInfo,
    },
};
use crate::msg::VestingInfo;

const DAO_ADDR: &str = "dao";
const CREATOR_ADDR: &str = "creator";

fn cw20_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    );
    Box::new(contract)
}

fn staking_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_stake::contract::execute,
        cw20_stake::contract::instantiate,
        cw20_stake::contract::query,
    );
    Box::new(contract)
}

fn vesting_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
      klmd_custom_vesting::contract::execute,
        klmd_custom_vesting::contract::instantiate,
        klmd_custom_vesting::contract::query,
    );
    Box::new(contract)
}

fn staked_balance_voting_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    )
        .with_reply(crate::contract::reply)
        .with_migrate(crate::contract::migrate);
    Box::new(contract)
}

fn instantiate_voting(app: &mut App, voting_id: u64, msg: InstantiateMsg) -> Addr {
    app.instantiate_contract(
        voting_id,
        Addr::unchecked(DAO_ADDR),
        &msg,
        &[],
        "voting module",
        None,
    )
        .unwrap()
}

fn stake_tokens(app: &mut App, staking_addr: Addr, cw20_addr: Addr, sender: &str, amount: u128) {
    let msg = cw20::Cw20ExecuteMsg::Send {
        contract: staking_addr.to_string(),
        amount: Uint128::new(amount),
        msg: to_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
    };
    app.execute_contract(Addr::unchecked(sender), cw20_addr, &msg, &[])
        .unwrap();
}

fn send_token(app: &mut App, receiver: &str, cw20_addr: Addr, sender: &str, amount: u128) {
    let msg = cw20::Cw20ExecuteMsg::Transfer {
        amount: Uint128::new(amount),
        recipient: receiver.to_string(),
    };
    app.execute_contract(Addr::unchecked(sender), cw20_addr, &msg, &[])
        .unwrap();
}

fn register_vesting_account(app: &mut App, account_addr: Addr, vesting_addr: Addr, sender: &str, prevesting_amount: Uint128, amount: Uint128, start_time: Timestamp, end_time: Timestamp) {
    let msg = klmd_custom_vesting::msg::ExecuteMsg::RegisterVestingAccount {
        address: account_addr.clone(),
        vesting_amount: amount,
        prevesting_amount: prevesting_amount,
        start_time,
        end_time,
    };
    app.execute_contract(Addr::unchecked(sender), vesting_addr, &msg, &[])
        .unwrap();
}

fn vesting_contract_snapshot(app: &mut App, vesting_addr: Addr, sender: &str) {
    let msg = klmd_custom_vesting::msg::ExecuteMsg::Snapshot {};
    app.execute_contract(Addr::unchecked(sender), vesting_addr, &msg, &[])
        .unwrap();
}

fn vesting_contract_claim(app: &mut App, vesting_addr: Addr, sender: &str) {
    let msg = klmd_custom_vesting::msg::ExecuteMsg::Claim {
        recipient: None,
    };
    app.execute_contract(Addr::unchecked(sender), vesting_addr, &msg, &[])
        .unwrap();
}

#[test]
#[should_panic(expected = "Initial governance token balances must not be empty")]
fn test_instantiate_zero_supply() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_contract());
    let voting_id = app.store_code(staked_balance_voting_contract());
    let staking_contract_id = app.store_code(staking_contract());
    let vesting_contract_id = app.store_code(vesting_contract());
    instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::zero(),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                vesting_code_id: vesting_contract_id,
                vesting_owner_address: None,
                initial_dao_balance: Some(Uint128::zero()),
            },
            active_threshold: None,
        },
    );
}

#[test]
#[should_panic(expected = "Initial governance token balances must not be empty")]
fn test_instantiate_no_balances() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_contract());
    let voting_id = app.store_code(staked_balance_voting_contract());
    let staking_contract_id = app.store_code(staking_contract());
    let vesting_contract_id = app.store_code(vesting_contract());
    instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                vesting_code_id: vesting_contract_id,
                vesting_owner_address: None,
                initial_dao_balance: Some(Uint128::zero()),
            },
            active_threshold: None,
        },
    );
}

#[test]
fn test_contract_info() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_contract());
    let voting_id = app.store_code(staked_balance_voting_contract());
    let staking_contract_id = app.store_code(staking_contract());
    let vesting_contract_id = app.store_code(vesting_contract());

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(2u64),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                vesting_code_id: vesting_contract_id,
                vesting_owner_address: None,
                initial_dao_balance: Some(Uint128::zero()),
            },
            active_threshold: None,
        },
    );

    let info: InfoResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::Info {})
        .unwrap();
    assert_eq!(
        info,
        InfoResponse {
            info: ContractVersion {
                contract: "crates.io:cw20-staked-balance-voting".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string()
            }
        }
    );

    let dao: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::Dao {})
        .unwrap();
    assert_eq!(dao, Addr::unchecked(DAO_ADDR));
}

#[test]
fn test_new_cw20() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_contract());
    let voting_id = app.store_code(staked_balance_voting_contract());
    let staking_contract_id = app.store_code(staking_contract());
    let vesting_contract_id = app.store_code(vesting_contract());

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(2u64),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                vesting_code_id: vesting_contract_id,
                vesting_owner_address: None,
                initial_dao_balance: Some(Uint128::from(10u64)),
            },
            active_threshold: None,
        },
    );

    let token_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::TokenContract {})
        .unwrap();
    let staking_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::StakingContract {})
        .unwrap();
    let vesting_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::VestingContract {})
        .unwrap();

    let token_info: TokenInfoResponse = app
        .wrap()
        .query_wasm_smart(token_addr.clone(), &cw20::Cw20QueryMsg::TokenInfo {})
        .unwrap();
    assert_eq!(
        token_info,
        TokenInfoResponse {
            name: "DAO DAO".to_string(),
            symbol: "DAO".to_string(),
            decimals: 6,
            total_supply: Uint128::from(12u64)
        }
    );

    let minter_info: Option<MinterResponse> = app
        .wrap()
        .query_wasm_smart(token_addr.clone(), &cw20::Cw20QueryMsg::Minter {})
        .unwrap();
    assert_eq!(
        minter_info,
        Some(MinterResponse {
            minter: DAO_ADDR.to_string(),
            cap: None,
        })
    );

    // Expect DAO (sender address) to have initial balance.
    let token_info: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token_addr.clone(),
            &cw20::Cw20QueryMsg::Balance {
                address: DAO_ADDR.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        token_info,
        BalanceResponse {
            balance: Uint128::from(10u64)
        }
    );

    // Expect 0 as they have not staked and not vested.
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::zero(),
            height: app.block_info().height,
        }
    );

    // Expect 0 as DAO has not staked and not vested.
    let dao_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: DAO_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        dao_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::zero(),
            height: app.block_info().height,
        }
    );

    // Stake 1 token as creator
    stake_tokens(&mut app, staking_addr, token_addr, CREATOR_ADDR, 1);
    app.update_block(next_block);

    // Expect 1 as creator has now staked 1 and not vested.
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height,
        }
    );

    // Expect 1 as only one token staked to make up whole voting power
    let total_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::TotalPowerAtHeight { height: None })
        .unwrap();

    assert_eq!(
        total_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height,
        }
    );

    // Vest 1 token as creator
    let curr_time: Timestamp = app.block_info().time;
    register_vesting_account(
        &mut app,
        Addr::unchecked("creator"),
        vesting_addr.clone(),
        "dao",
        Uint128::zero(),
        Uint128::new(1u128),
        curr_time,
        curr_time.plus_seconds(5),
    );
    app.update_block(next_block);

    // vesting snapshot is taken at the end of the block
    vesting_contract_snapshot(
        &mut app,
        vesting_addr,
        CREATOR_ADDR,
    );

    // Expect 2 as creator has now staked 1 and vested 1.
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(2u128),
            height: app.block_info().height,
        }
    );
}

#[test]
fn test_existing_cw20_new_staking_new_vesting() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_contract());
    let voting_id = app.store_code(staked_balance_voting_contract());
    let staking_id = app.store_code(staking_contract());
    let vesting_id = app.store_code(vesting_contract());

    let token_addr = app
        .instantiate_contract(
            cw20_id,
            Addr::unchecked(CREATOR_ADDR),
            &cw20_base::msg::InstantiateMsg {
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 3,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(2u64),
                }],
                mint: None,
                marketing: None,
            },
            &[],
            "voting token",
            None,
        )
        .unwrap();

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::Existing {
                address: token_addr.to_string(),
                staking_contract: StakingInfo::New {
                    staking_code_id: staking_id,
                    unstaking_duration: None,
                },
                vesting_contract: VestingInfo::New {
                    vesting_code_id: vesting_id,
                    owner_address: None,
                }
            },
            active_threshold: None,
        },
    );

    let token_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::TokenContract {})
        .unwrap();
    let staking_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::StakingContract {})
        .unwrap();

    let token_info: TokenInfoResponse = app
        .wrap()
        .query_wasm_smart(token_addr.clone(), &cw20::Cw20QueryMsg::TokenInfo {})
        .unwrap();
    assert_eq!(
        token_info,
        TokenInfoResponse {
            name: "DAO DAO".to_string(),
            symbol: "DAO".to_string(),
            decimals: 3,
            total_supply: Uint128::from(2u64)
        }
    );

    let minter_info: Option<MinterResponse> = app
        .wrap()
        .query_wasm_smart(token_addr.clone(), &cw20::Cw20QueryMsg::Minter {})
        .unwrap();
    assert!(minter_info.is_none());

    // Expect 0 as creator has not staked
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::zero(),
            height: app.block_info().height,
        }
    );

    // Expect 0 as DAO has not staked
    let dao_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: DAO_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        dao_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::zero(),
            height: app.block_info().height,
        }
    );

    // Stake 1 token as creator
    stake_tokens(&mut app, staking_addr, token_addr, CREATOR_ADDR, 1);
    app.update_block(next_block);

    // Expect 1 as creator has now staked 1
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height,
        }
    );

    // Expect 1 as only one token staked to make up whole voting power
    let total_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::TotalPowerAtHeight { height: None })
        .unwrap();

    assert_eq!(
        total_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height,
        }
    )
}

#[test]
fn test_existing_cw20_new_staking_existing_vesting() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_contract());
    let voting_id = app.store_code(staked_balance_voting_contract());
    let staking_id = app.store_code(staking_contract());
    let vesting_id = app.store_code(vesting_contract());

    let token_addr: Addr = app
        .instantiate_contract(
            cw20_id,
            Addr::unchecked(CREATOR_ADDR),
            &cw20_base::msg::InstantiateMsg {
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 3,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.clone().to_string(),
                    amount: Uint128::from(2u64),
                }],
                mint: None,
                marketing: None,
            },
            &[],
            "voting token",
            None,
        )
        .unwrap();

    let vesting_addr: Addr = app
        .instantiate_contract(
            vesting_id,
            Addr::unchecked(CREATOR_ADDR),
            &klmd_custom_vesting::msg::InstantiateMsg {
                owner_address: Some(Addr::unchecked(CREATOR_ADDR)),
                token_address: token_addr.clone(),
            },
            &[],
            "vesting contract",
            None,
        )
        .unwrap();

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::Existing {
                address: token_addr.to_string(),
                staking_contract: StakingInfo::New {
                    staking_code_id: staking_id,
                    unstaking_duration: None,
                },
                vesting_contract: VestingInfo::Existing {
                    vesting_contract_address: vesting_addr.to_string(),
                }
            },
            active_threshold: None,
        },
    );

    let token_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::TokenContract {})
        .unwrap();
    let staking_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::StakingContract {})
        .unwrap();
    let vesting_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::VestingContract {})
        .unwrap();

    let token_info: TokenInfoResponse = app
        .wrap()
        .query_wasm_smart(token_addr.clone(), &cw20::Cw20QueryMsg::TokenInfo {})
        .unwrap();
    assert_eq!(
        token_info,
        TokenInfoResponse {
            name: "DAO DAO".to_string(),
            symbol: "DAO".to_string(),
            decimals: 3,
            total_supply: Uint128::from(2u64)
        }
    );

    let minter_info: Option<MinterResponse> = app
        .wrap()
        .query_wasm_smart(token_addr.clone(), &cw20::Cw20QueryMsg::Minter {})
        .unwrap();
    assert!(minter_info.is_none());

    // Expect 0 as creator has not staked and not vested
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.clone().to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::zero(),
            height: app.block_info().height,
        }
    );

    // Expect 0 as DAO has not staked and not staked
    let dao_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: DAO_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        dao_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::zero(),
            height: app.block_info().height,
        }
    );

    // Stake 1 token as creator
    stake_tokens(&mut app, staking_addr, token_addr, CREATOR_ADDR, 1);
    app.update_block(next_block);

    // Expect 1 as creator has now staked 1
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height,
        }
    );

    // Expect 1 as only one token staked to make up whole voting power
    let total_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::TotalPowerAtHeight { height: None })
        .unwrap();

    assert_eq!(
        total_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height,
        }
    );

    // Vest 1 token as creator
    let curr_time: Timestamp = app.block_info().time;
    register_vesting_account(
        &mut app,
        Addr::unchecked(CREATOR_ADDR),
        vesting_addr.clone(),
        CREATOR_ADDR,
        Uint128::zero(),
        Uint128::new(1),
        curr_time.clone(),
        curr_time.plus_seconds(5),
    );

    app.update_block(next_block);
    vesting_contract_snapshot(&mut app, vesting_addr.clone(), CREATOR_ADDR);

    // Expect 2 as creator has now staked 1 and vested 1
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(2u128),
            height: app.block_info().height,
        }
    );

}

#[test]
fn test_existing_cw20_existing_staking_existing_vesting() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_contract());
    let voting_id = app.store_code(staked_balance_voting_contract());
    let staking_id = app.store_code(staking_contract());
    let vesting_id = app.store_code(vesting_contract());

    let token_addr = app
        .instantiate_contract(
            cw20_id,
            Addr::unchecked(CREATOR_ADDR),
            &cw20_base::msg::InstantiateMsg {
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 3,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(2u64),
                }],
                mint: None,
                marketing: None,
            },
            &[],
            "voting token",
            None,
        )
        .unwrap();

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::Existing {
                address: token_addr.to_string(),
                staking_contract: StakingInfo::New {
                    staking_code_id: staking_id,
                    unstaking_duration: None,
                },
                vesting_contract: VestingInfo::New {
                    vesting_code_id: vesting_id,
                    owner_address: None,
                },
            },
            active_threshold: None,
        },
    );

    let token_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::TokenContract {})
        .unwrap();
    // We'll use this for our valid existing contract
    let staking_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::StakingContract {})
        .unwrap();
    let vesting_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::VestingContract {})
        .unwrap();

    let token_info: TokenInfoResponse = app
        .wrap()
        .query_wasm_smart(token_addr.clone(), &cw20::Cw20QueryMsg::TokenInfo {})
        .unwrap();
    assert_eq!(
        token_info,
        TokenInfoResponse {
            name: "DAO DAO".to_string(),
            symbol: "DAO".to_string(),
            decimals: 3,
            total_supply: Uint128::from(2u64)
        }
    );

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::Existing {
                address: token_addr.to_string(),
                staking_contract: StakingInfo::Existing {
                    staking_contract_address: staking_addr.to_string(),
                },
                vesting_contract: VestingInfo::Existing {
                    vesting_contract_address: vesting_addr.to_string(),
                },
            },
            active_threshold: None,
        },
    );

    // Expect 0 as creator has not staked
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::zero(),
            height: app.block_info().height,
        }
    );

    // Expect 0 as DAO has not staked
    let dao_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: DAO_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        dao_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::zero(),
            height: app.block_info().height,
        }
    );

    // Stake 1 token as creator
    stake_tokens(&mut app, staking_addr.clone(), token_addr, CREATOR_ADDR, 1);
    app.update_block(next_block);

    // Expect 1 as creator has now staked 1
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height,
        }
    );

    // Expect 1 as only one token staked to make up whole voting power
    let total_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::TotalPowerAtHeight { height: None })
        .unwrap();

    assert_eq!(
        total_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height,
        }
    );

    // Now lets test the error case where we use an invalid staking contract
    let different_token = app
        .instantiate_contract(
            cw20_id,
            Addr::unchecked(CREATOR_ADDR),
            &cw20_base::msg::InstantiateMsg {
                name: "DAO DAO MISMATCH".to_string(),
                symbol: "DAOM".to_string(),
                decimals: 3,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(2u64),
                }],
                mint: None,
                marketing: None,
            },
            &[],
            "voting token",
            None,
        )
        .unwrap();

    // Expect error as the token address does not match the staking address token address
    app.instantiate_contract(
        voting_id,
        Addr::unchecked(DAO_ADDR),
        &InstantiateMsg {
            token_info: crate::msg::TokenInfo::Existing {
                address: different_token.to_string(),
                staking_contract: StakingInfo::Existing {
                    staking_contract_address: staking_addr.to_string(),
                },
                vesting_contract: VestingInfo::Existing {
                    vesting_contract_address: vesting_addr.to_string(),
                },
            },
            active_threshold: None,
        },
        &[],
        "voting module",
        None,
    )
        .unwrap_err();
}

#[test]
fn test_different_heights() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_contract());
    let voting_id = app.store_code(staked_balance_voting_contract());
    let staking_id = app.store_code(staking_contract());
    let vesting_id = app.store_code(vesting_contract());

    let token_addr = app
        .instantiate_contract(
            cw20_id,
            Addr::unchecked(CREATOR_ADDR),
            &cw20_base::msg::InstantiateMsg {
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 3,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(2u64),
                }],
                mint: None,
                marketing: None,
            },
            &[],
            "voting token",
            None,
        )
        .unwrap();

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::Existing {
                address: token_addr.to_string(),
                staking_contract: StakingInfo::New {
                    staking_code_id: staking_id,
                    unstaking_duration: None,
                },
                vesting_contract: VestingInfo::New {
                    vesting_code_id: vesting_id,
                    owner_address: None
                },
            },
            active_threshold: None,
        },
    );

    let token_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::TokenContract {})
        .unwrap();
    let staking_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::StakingContract {})
        .unwrap();

    // Expect 0 as creator has not staked
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::zero(),
            height: app.block_info().height,
        }
    );

    // Stake 1 token as creator
    stake_tokens(
        &mut app,
        staking_addr.clone(),
        token_addr.clone(),
        CREATOR_ADDR,
        1,
    );
    app.update_block(next_block);

    // Expect 1 as creator has now staked 1
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height,
        }
    );

    // Expect 1 as only one token staked to make up whole voting power
    let total_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::TotalPowerAtHeight { height: None },
        )
        .unwrap();

    assert_eq!(
        total_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height,
        }
    );

    // Stake another 1 token as creator
    stake_tokens(&mut app, staking_addr, token_addr, CREATOR_ADDR, 1);
    app.update_block(next_block);

    // Expect 2 as creator has now staked 2
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(2u128),
            height: app.block_info().height,
        }
    );

    // Expect 2 as we have now staked 2
    let total_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::TotalPowerAtHeight { height: None },
        )
        .unwrap();

    assert_eq!(
        total_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(2u128),
            height: app.block_info().height,
        }
    );

    // Check we can query history
    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr.clone(),
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: Some(app.block_info().height - 1),
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height - 1,
        }
    );

    // Expect 1 at the old height prior to second stake
    let total_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            voting_addr,
            &QueryMsg::TotalPowerAtHeight {
                height: Some(app.block_info().height - 1),
            },
        )
        .unwrap();

    assert_eq!(
        total_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::new(1u128),
            height: app.block_info().height - 1,
        }
    );
}

#[test]
fn test_active_threshold_absolute_count() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_contract());
    let voting_id = app.store_code(staked_balance_voting_contract());
    let staking_contract_id = app.store_code(staking_contract());
    let vesting_contract_id = app.store_code(vesting_contract());

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(200u64),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                vesting_code_id: vesting_contract_id,
                vesting_owner_address: None,
                initial_dao_balance: Some(Uint128::from(100u64)),
            },
            active_threshold: Some(ActiveThreshold::AbsoluteCount {
                count: Uint128::new(100),
            }),
        },
    );

    let token_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::TokenContract {})
        .unwrap();
    let staking_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::StakingContract {})
        .unwrap();

    // Not active as none staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::IsActive {})
        .unwrap();
    assert!(!is_active.active);

    // Stake 100 token as creator
    stake_tokens(&mut app, staking_addr, token_addr, CREATOR_ADDR, 100);
    app.update_block(next_block);

    // Active as enough staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::IsActive {})
        .unwrap();
    assert!(is_active.active);
}

#[test]
fn test_active_threshold_percent() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_contract());
    let voting_id = app.store_code(staked_balance_voting_contract());
    let staking_contract_id = app.store_code(staking_contract());
    let vesting_contract_id = app.store_code(vesting_contract());

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(200u64),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                vesting_code_id: vesting_contract_id,
                vesting_owner_address: None,
                initial_dao_balance: Some(Uint128::from(100u64)),
            },
            active_threshold: Some(ActiveThreshold::Percentage {
                percent: Decimal::percent(20),
            }),
        },
    );

    let token_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::TokenContract {})
        .unwrap();
    let staking_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::StakingContract {})
        .unwrap();

    // Not active as none staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::IsActive {})
        .unwrap();
    assert!(!is_active.active);

    // Stake 60 token as creator, now active
    stake_tokens(&mut app, staking_addr, token_addr, CREATOR_ADDR, 60);
    app.update_block(next_block);

    // Active as enough staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::IsActive {})
        .unwrap();
    assert!(is_active.active);
}

#[test]
fn test_active_threshold_percent_rounds_up() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_contract());
    let voting_id = app.store_code(staked_balance_voting_contract());
    let staking_contract_id = app.store_code(staking_contract());
    let vesting_contract_id = app.store_code(vesting_contract());

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(5u64),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                vesting_code_id: vesting_contract_id,
                vesting_owner_address: None,
                initial_dao_balance: None,
            },
            active_threshold: Some(ActiveThreshold::Percentage {
                percent: Decimal::percent(50),
            }),
        },
    );

    let token_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::TokenContract {})
        .unwrap();
    let staking_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::StakingContract {})
        .unwrap();

    // Not active as none staked
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::IsActive {})
        .unwrap();
    assert!(!is_active.active);

    // Stake 2 token as creator, should not be active.
    stake_tokens(
        &mut app,
        staking_addr.clone(),
        token_addr.clone(),
        CREATOR_ADDR,
        2,
    );
    app.update_block(next_block);

    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::IsActive {})
        .unwrap();
    assert!(!is_active.active);

    // Stake 1 more token as creator, should now be active.
    stake_tokens(&mut app, staking_addr, token_addr, CREATOR_ADDR, 2);
    app.update_block(next_block);

    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::IsActive {})
        .unwrap();
    assert!(is_active.active);
}

#[test]
fn test_active_threshold_none() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_contract());
    let voting_id = app.store_code(staked_balance_voting_contract());
    let staking_contract_id = app.store_code(staking_contract());
    let vesting_contract_id = app.store_code(vesting_contract());

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(200u64),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                vesting_code_id: vesting_contract_id,
                vesting_owner_address: None,
                initial_dao_balance: Some(Uint128::from(100u64)),
            },
            active_threshold: None,
        },
    );

    // Active as no threshold
    let is_active: IsActiveResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::IsActive {})
        .unwrap();
    assert!(is_active.active);
}

#[test]
fn test_update_active_threshold() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_contract());
    let voting_id = app.store_code(staked_balance_voting_contract());
    let staking_contract_id = app.store_code(staking_contract());
    let vesting_contract_id = app.store_code(vesting_contract());

    let voting_addr = instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(200u64),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                vesting_code_id: vesting_contract_id,
                vesting_owner_address: None,
                initial_dao_balance: Some(Uint128::from(100u64)),
            },
            active_threshold: None,
        },
    );

    let resp: ActiveThresholdResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::ActiveThreshold {})
        .unwrap();
    assert_eq!(resp.active_threshold, None);

    let msg = ExecuteMsg::UpdateActiveThreshold {
        new_threshold: Some(ActiveThreshold::AbsoluteCount {
            count: Uint128::new(100),
        }),
    };

    // Expect failure as sender is not the DAO
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        voting_addr.clone(),
        &msg,
        &[],
    )
        .unwrap_err();

    // Expect success as sender is the DAO
    app.execute_contract(Addr::unchecked(DAO_ADDR), voting_addr.clone(), &msg, &[])
        .unwrap();

    let resp: ActiveThresholdResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::ActiveThreshold {})
        .unwrap();
    assert_eq!(
        resp.active_threshold,
        Some(ActiveThreshold::AbsoluteCount {
            count: Uint128::new(100)
        })
    );
}

#[test]
#[should_panic(expected = "Active threshold percentage must be greater than 0 and less than 1")]
fn test_active_threshold_percentage_gt_100() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_contract());
    let voting_id = app.store_code(staked_balance_voting_contract());
    let staking_contract_id = app.store_code(staking_contract());
    let vesting_contract_id = app.store_code(vesting_contract());

    instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(200u64),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                vesting_code_id: vesting_contract_id,
                vesting_owner_address: None,
                initial_dao_balance: Some(Uint128::from(100u64)),
            },
            active_threshold: Some(ActiveThreshold::Percentage {
                percent: Decimal::percent(120),
            }),
        },
    );
}

#[test]
#[should_panic(expected = "Active threshold percentage must be greater than 0 and less than 1")]
fn test_active_threshold_percentage_lte_0() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_contract());
    let voting_id = app.store_code(staked_balance_voting_contract());
    let staking_contract_id = app.store_code(staking_contract());
    let vesting_contract_id = app.store_code(vesting_contract());

    instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(200u64),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                vesting_code_id: vesting_contract_id,
                vesting_owner_address: None,
                initial_dao_balance: Some(Uint128::from(100u64)),
            },
            active_threshold: Some(ActiveThreshold::Percentage {
                percent: Decimal::percent(0),
            }),
        },
    );
}

#[test]
#[should_panic(expected = "Absolute count threshold cannot be greater than the total token supply")]
fn test_active_threshold_absolute_count_invalid() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_contract());
    let voting_id = app.store_code(staked_balance_voting_contract());
    let staking_contract_id = app.store_code(staking_contract());
    let vesting_contract_id = app.store_code(vesting_contract());

    instantiate_voting(
        &mut app,
        voting_id,
        InstantiateMsg {
            token_info: crate::msg::TokenInfo::New {
                code_id: cw20_id,
                label: "DAO DAO voting".to_string(),
                name: "DAO DAO".to_string(),
                symbol: "DAO".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: CREATOR_ADDR.to_string(),
                    amount: Uint128::from(200u64),
                }],
                marketing: None,
                unstaking_duration: None,
                staking_code_id: staking_contract_id,
                vesting_code_id: vesting_contract_id,
                vesting_owner_address: None,
                initial_dao_balance: Some(Uint128::from(100u64)),
            },
            active_threshold: Some(ActiveThreshold::AbsoluteCount {
                count: Uint128::new(10000),
            }),
        },
    );
}

#[test]
fn test_migrate() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_contract());
    let voting_id = app.store_code(staked_balance_voting_contract());
    let staking_contract_id = app.store_code(staking_contract());
    let vesting_contract_id = app.store_code(vesting_contract());

    let voting_addr = app
        .instantiate_contract(
            voting_id,
            Addr::unchecked(DAO_ADDR),
            &InstantiateMsg {
                token_info: crate::msg::TokenInfo::New {
                    code_id: cw20_id,
                    label: "DAO DAO voting".to_string(),
                    name: "DAO DAO".to_string(),
                    symbol: "DAO".to_string(),
                    decimals: 6,
                    initial_balances: vec![Cw20Coin {
                        address: CREATOR_ADDR.to_string(),
                        amount: Uint128::from(2u64),
                    }],
                    marketing: None,
                    unstaking_duration: None,
                    staking_code_id: staking_contract_id,
                    vesting_code_id: vesting_contract_id,
                    vesting_owner_address: None,
                    initial_dao_balance: Some(Uint128::zero()),
                },
                active_threshold: None,
            },
            &[],
            "voting module",
            Some(DAO_ADDR.to_string()),
        )
        .unwrap();

    let info: InfoResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::Info {})
        .unwrap();

    let dao: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::Dao {})
        .unwrap();

    app.execute(
        dao,
        CosmosMsg::Wasm(WasmMsg::Migrate {
            contract_addr: voting_addr.to_string(),
            new_code_id: voting_id,
            msg: to_binary(&MigrateMsg {}).unwrap(),
        }),
    )
        .unwrap();

    let new_info: InfoResponse = app
        .wrap()
        .query_wasm_smart(voting_addr, &QueryMsg::Info {})
        .unwrap();

    assert_eq!(info, new_info);
}

#[test]
pub fn test_migrate_update_version() {
    let mut deps = mock_dependencies();
    cw2::set_contract_version(&mut deps.storage, "my-contract", "old-version").unwrap();
    migrate(deps.as_mut(), mock_env(), MigrateMsg {}).unwrap();
    let version = cw2::get_contract_version(&deps.storage).unwrap();
    assert_eq!(version.version, CONTRACT_VERSION);
    assert_eq!(version.contract, CONTRACT_NAME);
}

// KLEO CUSTOM TESTS

fn dao_core_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw_core::contract::execute,
        cw_core::contract::instantiate,
        cw_core::contract::query,
    )
        .with_reply(cw_core::contract::reply);
    Box::new(contract)
}

fn proposal_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw_proposal_single::contract::execute,
        cw_proposal_single::contract::instantiate,
        cw_proposal_single::contract::query,
    ).with_reply(cw_proposal_single::contract::reply);
    Box::new(contract)
}

fn instantiate_dao(app: &mut App, dao_core_id: u64, msg: cw_core::msg::InstantiateMsg) -> Addr {
    app.instantiate_contract(
        dao_core_id,
        Addr::unchecked(CREATOR_ADDR),
        &msg,
        &[],
        "dao core module",
        None,
    )
        .unwrap()
}

fn create_proposal(app: &mut App, proposal_addr: Addr, sender: &str) {
    let msg = cw_proposal_single::msg::ExecuteMsg::Propose {
        title: "test proposal".to_string(),
        description: "test description".to_string(),
        msgs: vec![],
    };
    app.execute_contract(Addr::unchecked(sender), proposal_addr, &msg, &[])
        .unwrap();
}

#[test]
pub fn test_contracts_integration_init() {
    let mut app = App::default();
    let dao_core_id = app.store_code(dao_core_contract());
    let proposal_id = app.store_code(proposal_contract());
    let cw20_id = app.store_code(cw20_contract());
    let voting_id = app.store_code(staked_balance_voting_contract());
    let staking_contract_id = app.store_code(staking_contract());
    let vesting_contract_id = app.store_code(vesting_contract());

    let dao_core_addr = instantiate_dao(
        &mut app,
        dao_core_id,
        cw_core::msg::InstantiateMsg {
            admin: Some(CREATOR_ADDR.to_string()),
            name: "KLEO TEST DAO".to_string(),
            description: "KLEO TEST DAO DESCR".to_string(),
            image_url: None,
            automatically_add_cw20s: false,
            automatically_add_cw721s: false,
            voting_module_instantiate_info: cw_core::msg::ModuleInstantiateInfo {
                code_id: voting_id,
                msg: to_binary(&InstantiateMsg {
                    token_info: crate::msg::TokenInfo::New {
                        code_id: cw20_id,
                        label: "DAO DAO voting".to_string(),
                        name: "DAO DAO".to_string(),
                        symbol: "DAO".to_string(),
                        decimals: 6,
                        initial_balances: vec![Cw20Coin {
                            address: CREATOR_ADDR.to_string(),
                            amount: Uint128::from(200u128),
                        }],
                        marketing: None,
                        unstaking_duration: None,
                        staking_code_id: staking_contract_id,
                        vesting_code_id: vesting_contract_id,
                        vesting_owner_address: Some(CREATOR_ADDR.to_string()),
                        initial_dao_balance: Some(Uint128::zero()),
                    },
                    active_threshold: None,
                }).unwrap(),
                admin: cw_core::msg::Admin::CoreContract {},
                label: "voting module".to_string()
            },
            proposal_modules_instantiate_info: vec![
                cw_core::msg::ModuleInstantiateInfo {
                    code_id: proposal_id,
                    msg: to_binary(&cw_proposal_single::msg::InstantiateMsg {
                        threshold: voting::Threshold::AbsolutePercentage { percentage: voting::PercentageThreshold::Majority {}},
                        max_voting_period: Duration::Height(5),
                        min_voting_period: None,
                        only_members_execute: true,
                        allow_revoting: false,
                        deposit_info: None
                    }).unwrap(),
                    admin: cw_core::msg::Admin::CoreContract {},
                    label: "proposal module".to_string()
                }
            ],
            initial_items: None
        },
    );

    let voting_addr: Addr = app
        .wrap()
        .query_wasm_smart(dao_core_addr.clone(), &cw_core::msg::QueryMsg::VotingModule {})
        .unwrap();

    let staking_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::StakingContract {})
        .unwrap();

    let cw20_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::TokenContract {})
        .unwrap();

    let vesting_addr: Addr = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::VestingContract {})
        .unwrap();

    let proposal_addr: Vec<Addr> = app
        .wrap()
        .query_wasm_smart(dao_core_addr.clone(), &cw_core::msg::QueryMsg::ProposalModules {
            start_at: None,
            limit: None,
        })
        .unwrap();

    // register snapshot for vesting on proposal module
    let proposal_hook_msg = cw_proposal_single::msg::ExecuteMsg::AddProposalHook {
        address: vesting_addr.clone().to_string(),
    };
    app.execute_contract(Addr::unchecked(dao_core_addr), proposal_addr[0].clone(), &proposal_hook_msg, &[])
        .unwrap();

    send_token(&mut app, vesting_addr.clone().as_str(), cw20_addr.clone(), CREATOR_ADDR, 100u128);

    assert_eq!(voting_addr, Addr::unchecked("contract1"));
    assert_eq!(cw20_addr, Addr::unchecked("contract2"));
    assert_eq!(staking_addr, Addr::unchecked("contract3"));
    assert_eq!(vesting_addr, Addr::unchecked("contract4"));
    assert_eq!(proposal_addr, vec![Addr::unchecked("contract5")]);

    let voting_power_response: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::VotingPowerAtHeight {
            address: CREATOR_ADDR.to_string(),
            height: None,
        })
        .unwrap();

    assert_eq!(voting_power_response, VotingPowerAtHeightResponse {
        power: Uint128::new(0),
        height: app.block_info().height
    });

    // boostrap vesting
    let time_zero: Timestamp = app.block_info().time;
    register_vesting_account(
        &mut app,
        Addr::unchecked(CREATOR_ADDR),
        vesting_addr.clone(),
        CREATOR_ADDR,
        Uint128::new(10),
        Uint128::new(100),
        time_zero.plus_seconds(5),
        time_zero.plus_seconds(105),
    );

    let voting_power_response: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::VotingPowerAtHeight {
            address: CREATOR_ADDR.to_string(),
            height: None,
        })
        .unwrap();

    assert_eq!(voting_power_response, VotingPowerAtHeightResponse {
        power: Uint128::new(10),
        height: app.block_info().height
    });

    // time 1 the prevesting is over so the power should be 100

    app.update_block(next_block);
    vesting_contract_snapshot(&mut app, vesting_addr.clone(), proposal_addr[0].clone().as_str());
    let vesting_account_response: klmd_custom_vesting::msg::VestingAccountResponse = app
        .wrap()
        .query_wasm_smart(vesting_addr.clone(), &klmd_custom_vesting::msg::QueryMsg::VestingAccount {
            address: Addr::unchecked(CREATOR_ADDR),
            height: None,
        })
        .unwrap();

    assert_eq!(
        vesting_account_response,
        klmd_custom_vesting::msg::VestingAccountResponse {
            address: Addr::unchecked(CREATOR_ADDR),
            vestings: klmd_custom_vesting::state::VestingData {
                prevesting_amount: Uint128::new(10),
                prevested_amount: Uint128::new(100),
                vesting_amount: Uint128::new(100),
                vested_amount: Uint128::zero(),
                claimable_amount: Uint128::zero(),
                claimed_amount: Uint128::zero(),
                registration_time: time_zero,
                start_time: time_zero.plus_seconds(5),
                end_time: time_zero.plus_seconds(105),
            }
        }
    );

    let voting_power_response: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::VotingPowerAtHeight {
            address: CREATOR_ADDR.to_string(),
            height: None,
        })
        .unwrap();

    assert_eq!(voting_power_response, VotingPowerAtHeightResponse {
        power: Uint128::new(100),
        height: app.block_info().height
    });

    // time 2 the vesting is progressing so the power should be 100 because no claim happened
    app.update_block(next_block);
    vesting_contract_snapshot(&mut app, vesting_addr.clone(), proposal_addr[0].clone().as_str());
    let vesting_account_response: klmd_custom_vesting::msg::VestingAccountResponse = app
        .wrap()
        .query_wasm_smart(vesting_addr.clone(), &klmd_custom_vesting::msg::QueryMsg::VestingAccount {
            address: Addr::unchecked(CREATOR_ADDR),
            height: None,
        })
        .unwrap();

    assert_eq!(
        vesting_account_response,
        klmd_custom_vesting::msg::VestingAccountResponse {
            address: Addr::unchecked(CREATOR_ADDR),
            vestings: klmd_custom_vesting::state::VestingData {
                prevesting_amount: Uint128::new(10),
                prevested_amount: Uint128::new(100),
                vesting_amount: Uint128::new(100),
                vested_amount: Uint128::new(5),
                claimable_amount: Uint128::new(5),
                claimed_amount: Uint128::zero(),
                registration_time: time_zero,
                start_time: time_zero.plus_seconds(5),
                end_time: time_zero.plus_seconds(105),
            }
        }
    );

    let voting_power_response: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::VotingPowerAtHeight {
            address: CREATOR_ADDR.to_string(),
            height: None,
        })
        .unwrap();

    assert_eq!(voting_power_response, VotingPowerAtHeightResponse {
        power: Uint128::new(100),
        height: app.block_info().height
    });

    // time 3 the vesting is progressing but the claim happened so the power should be 90 (10 vested and claimed)
    app.update_block(next_block);
    vesting_contract_claim(&mut app, vesting_addr.clone(), CREATOR_ADDR);
    let vesting_account_response: klmd_custom_vesting::msg::VestingAccountResponse = app
        .wrap()
        .query_wasm_smart(vesting_addr.clone(), &klmd_custom_vesting::msg::QueryMsg::VestingAccount {
            address: Addr::unchecked(CREATOR_ADDR),
            height: None,
        })
        .unwrap();

    assert_eq!(
        vesting_account_response,
        klmd_custom_vesting::msg::VestingAccountResponse {
            address: Addr::unchecked(CREATOR_ADDR),
            vestings: klmd_custom_vesting::state::VestingData {
                prevesting_amount: Uint128::new(10),
                prevested_amount: Uint128::new(100),
                vesting_amount: Uint128::new(100),
                vested_amount: Uint128::new(10),
                claimable_amount: Uint128::new(0),
                claimed_amount: Uint128::new(10),
                registration_time: time_zero,
                start_time: time_zero.plus_seconds(5),
                end_time: time_zero.plus_seconds(105),
            }
        }
    );

    let voting_power_response: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::VotingPowerAtHeight {
            address: CREATOR_ADDR.to_string(),
            height: None,
        })
        .unwrap();

    assert_eq!(voting_power_response, VotingPowerAtHeightResponse {
        power: Uint128::new(90),
        height: app.block_info().height
    });

    let token_info: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            cw20_addr.clone(),
            &cw20::Cw20QueryMsg::Balance {
                address: CREATOR_ADDR.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        token_info,
        BalanceResponse {
            balance: Uint128::from(110u64)
        }
    );

    // time 4&5 the creator has 90 power from vesting tokens and it stakes 10 more tokens so the power should be 100
    app.update_block(next_block);
    stake_tokens(&mut app, staking_addr.clone(), cw20_addr.clone(), CREATOR_ADDR, 10u128);

    let token_info: BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            cw20_addr.clone(),
            &cw20::Cw20QueryMsg::Balance {
                address: CREATOR_ADDR.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        token_info,
        BalanceResponse {
            balance: Uint128::from(100u64)
        }
    );

    // next block to propagate the staking
    app.update_block(next_block);

    let staking_tokens: cw20_stake::msg::StakedBalanceAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            staking_addr.clone(),
            &cw20_stake::msg::QueryMsg::StakedBalanceAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        staking_tokens,
        cw20_stake::msg::StakedBalanceAtHeightResponse {
            balance: Uint128::from(10u64),
            height: app.block_info().height
        }
    );

    let voting_power_response: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::VotingPowerAtHeight {
            address: CREATOR_ADDR.to_string(),
            height: None,
        })
        .unwrap();

    assert_eq!(voting_power_response, VotingPowerAtHeightResponse {
        power: Uint128::new(100),
        height: app.block_info().height
    });

    let total_voting_power: TotalPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(voting_addr.clone(), &QueryMsg::TotalPowerAtHeight {
            height: None,
        })
        .unwrap();

    assert_eq!(total_voting_power, TotalPowerAtHeightResponse {
        power: Uint128::new(100),
        height: app.block_info().height
    });

    // time 6 create first proposal
    app.update_block(next_block);
    create_proposal(&mut app, proposal_addr[0].clone(), CREATOR_ADDR);

}
