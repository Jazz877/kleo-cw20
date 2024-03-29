use cosmwasm_std::{Addr, BlockInfo, coin, CosmosMsg, Empty, Timestamp, to_binary, Uint128, WasmMsg};
use cw20::{BalanceResponse, Cw20Coin, Cw20ExecuteMsg, Cw20QueryMsg};
use cw_multi_test::{App, Contract, ContractWrapper, Executor, next_block};

use crate::{contract, msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, VestingAccountResponse}, state::VestingData};
use crate::msg::InfoResponse;

const OWNER: &str = "owner0000";
const INITIAL_BALANCE: u128 = 100_000_000;
const USER1: &str = "user0001";



pub fn contract_cw20() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    );
    Box::new(contract)
}

pub fn contract_vesting() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        contract::execute,
        contract::instantiate,
        contract::query,
    ).with_migrate(crate::contract::migrate);
    Box::new(contract)
}

fn mock_app() -> App {
    let init_funds = vec![coin(20, "juno")];

    let app = App::new(|router, _, storage| {
        router
            .bank
            .init_balance(storage, &Addr::unchecked(&OWNER.to_string()), init_funds)
            .unwrap();
    });
    app
}

fn instantiate_cw20(app: &mut App) -> Addr {
    let cw20_code_id = app.store_code(contract_cw20());
    let msg = cw20_base::msg::InstantiateMsg {
        name: String::from("Kleomedes"),
        symbol: String::from("KLEO"),
        decimals: 6,
        initial_balances: vec![
            Cw20Coin {
                address: OWNER.to_string(),
                amount: Uint128::new(INITIAL_BALANCE),
            }
        ],
        mint: None,
        marketing: None,
    };
    app.instantiate_contract(cw20_code_id, Addr::unchecked(OWNER), &msg, &[], "cw20", None)
        .unwrap()
}

fn instantiate_vesting(app: &mut App, token_address: Addr) -> Addr {
    let vesting_code_id = app.store_code(contract_vesting());
    let msg = InstantiateMsg {
        token_address: token_address,
        owner_address: Some(Addr::unchecked(OWNER)),
    };
    app.instantiate_contract(vesting_code_id, Addr::unchecked(OWNER), &msg, &[], "vesting", Some(OWNER.to_string()))
        .unwrap()
}

fn query_cw20_balance(app: &App, cw20_addr: Addr, address: Addr) -> Uint128 {
    let msg = Cw20QueryMsg::Balance {address: address.into_string()};
    let balance_response: BalanceResponse = app
        .wrap()
        .query_wasm_smart(cw20_addr, &msg).unwrap();

    balance_response.balance
}

fn jump_100_blocks(block: &mut BlockInfo) {
    block.time = block.time.plus_seconds(500);
    block.height += 100;
}

#[test]
fn simple_e2e_test() {
    let mut app = mock_app();
    let cw20_contract_addr = instantiate_cw20(&mut app);
    let vesting_contract_addr = instantiate_vesting(&mut app, cw20_contract_addr.clone());

    let initial_owner_balance = Uint128::new(INITIAL_BALANCE);

    println!("{:?}", cw20_contract_addr.clone());
    println!("{:?}", vesting_contract_addr.clone());

    let initial_block_time = app.block_info().time;

    // move kleo on vesting contract
    let _ = app.execute_contract(
        Addr::unchecked(OWNER.to_string()),
        cw20_contract_addr.clone(),
        &Cw20ExecuteMsg::Transfer {
            recipient: vesting_contract_addr.clone().into_string(),
            amount: Uint128::new(10_000_000u128),
        },
        &vec![],
    );

    let owner_balance = query_cw20_balance(&app, cw20_contract_addr.clone(), Addr::unchecked(OWNER.to_string()));
    assert_eq!(initial_owner_balance.checked_sub(Uint128::new(10_000_000u128)).unwrap(), owner_balance);

    let vesting_balance = query_cw20_balance(&app, cw20_contract_addr.clone(), vesting_contract_addr.clone());
    assert_eq!(Uint128::new(10_000_000u128), vesting_balance);

    // register vesting account
    let _ = app.execute_contract(
        Addr::unchecked(OWNER.to_string()),
        vesting_contract_addr.clone(),
        &ExecuteMsg::RegisterVestingAccount {
            address: Addr::unchecked(USER1.to_string()),
            vesting_amount: Uint128::new(10_000_000),
            prevesting_amount: Uint128::new(1_000_000),
            start_time: initial_block_time,
            end_time: initial_block_time.plus_seconds(100),
        },
        &vec![],
    );

    // 5seconds more
    app.update_block(next_block);

    // fire snapshot
    let _ = app.execute_contract(
        Addr::unchecked(OWNER.to_string()),
        vesting_contract_addr.clone(),
        &ExecuteMsg::Snapshot {},
        &vec![],
    );


    let msg = QueryMsg::VestingAccount {
        address: Addr::unchecked(USER1.to_string()), height: Some(app.block_info().height),
    };

    let res: VestingAccountResponse = app.wrap().query_wasm_smart(vesting_contract_addr.clone(), &msg).unwrap();

    assert_eq!(
        VestingAccountResponse {
            address: Addr::unchecked(USER1.to_string()),
            vestings: VestingData {
                prevesting_amount: Uint128::new(1_000_000u128),
                prevested_amount: Uint128::new(10_000_000u128),
                vesting_amount: Uint128::new(10_000_000u128),
                vested_amount: Uint128::new(500_000u128),
                claimable_amount: Uint128::new(500_000u128),
                claimed_amount: Uint128::zero(),
                registration_time: initial_block_time,
                start_time: initial_block_time,
                end_time: initial_block_time.plus_seconds(100u64),
            }
        },
        res
    );

    // user1 claims tokens
    let _ = app.execute_contract(
        Addr::unchecked(USER1.to_string()),
        vesting_contract_addr.clone(),
        &ExecuteMsg::Claim {
            recipient: None,
        },
        &vec![],
    );

    let user1_balance = query_cw20_balance(&app, cw20_contract_addr.clone(), Addr::unchecked(USER1.to_string()));
    assert_eq!(Uint128::new(500_000u128), user1_balance);

    // 5seconds more

    app.update_block(next_block);

    let msg = QueryMsg::VestingAccount {
        address: Addr::unchecked(USER1.to_string()), height: Some(app.block_info().height),
    };

    let res: VestingAccountResponse = app.wrap().query_wasm_smart(vesting_contract_addr.clone(), &msg).unwrap();

    assert_eq!(
        VestingAccountResponse {
            address: Addr::unchecked(USER1.to_string()),
            vestings: VestingData {
                prevesting_amount: Uint128::new(1_000_000u128),
                prevested_amount: Uint128::new(10_000_000u128),
                vesting_amount: Uint128::new(10_000_000u128),
                vested_amount: Uint128::new(500_000u128),
                claimable_amount: Uint128::new(0),
                claimed_amount: Uint128::new(500_000u128),
                registration_time: initial_block_time,
                start_time: initial_block_time,
                end_time: initial_block_time.plus_seconds(100u64),
            }
        },
        res
    );

    // after some time
    app.update_block(jump_100_blocks);
    let _ = app.execute_contract(
        Addr::unchecked(USER1.to_string()),
        vesting_contract_addr.clone(),
        &ExecuteMsg::Claim {
            recipient: None,
        },
        &vec![],
    );

    let msg = QueryMsg::VestingAccount {
        address: Addr::unchecked(USER1.to_string()), height: Some(app.block_info().height),
    };

    let res: VestingAccountResponse = app.wrap().query_wasm_smart(vesting_contract_addr.clone(), &msg).unwrap();

    assert_eq!(
        VestingAccountResponse {
            address: Addr::unchecked(USER1.to_string()),
            vestings: VestingData {
                prevesting_amount: Uint128::new(1_000_000u128),
                prevested_amount: Uint128::new(10_000_000u128),
                vesting_amount: Uint128::new(10_000_000u128),
                vested_amount: Uint128::new(10_000_000u128),
                claimable_amount: Uint128::new(0),
                claimed_amount: Uint128::new(10_000_000u128),
                registration_time: initial_block_time,
                start_time: initial_block_time,
                end_time: initial_block_time.plus_seconds(100u64),
            }
        },
        res
    );


    // deregister user1

    //5seconds after
    app.update_block(next_block);

    let _ = app.execute_contract(
        Addr::unchecked(OWNER.to_string()),
        vesting_contract_addr.clone(),
        &ExecuteMsg::DeregisterVestingAccount {
            address: Addr::unchecked(USER1.to_string()),
            vested_token_recipient: Some(Addr::unchecked(USER1.to_string())),
            left_vesting_token_recipient: Some(Addr::unchecked(OWNER.to_string())),
        },
        &vec![],
    );

    let user1_balance = query_cw20_balance(&app, cw20_contract_addr.clone(), Addr::unchecked(USER1.to_string()));
    assert_eq!(Uint128::new(10_000_000u128), user1_balance);

    let owner_balance = query_cw20_balance(&app, cw20_contract_addr.clone(), Addr::unchecked(OWNER.to_string()));
    assert_eq!(Uint128::new(90_000_000), owner_balance);
}

#[test]
fn test_migrate() {
    let mut app = mock_app();
    let cw20_contract_addr = instantiate_cw20(&mut app);
    let vesting_contract_addr = instantiate_vesting(&mut app, cw20_contract_addr.clone());

    let new_vesting_contract_id = app.store_code(contract_vesting());

    let info: InfoResponse = app
        .wrap()
        .query_wasm_smart(vesting_contract_addr.clone(), &QueryMsg::Info {})
        .unwrap();

    app.execute(
        Addr::unchecked(OWNER),
        CosmosMsg::Wasm(WasmMsg::Migrate {
            contract_addr: vesting_contract_addr.clone().to_string(),
            new_code_id: new_vesting_contract_id,
            msg: to_binary(&MigrateMsg {}).unwrap(),
        }),
    )
        .unwrap();

    let new_info: InfoResponse = app
        .wrap()
        .query_wasm_smart(vesting_contract_addr, &QueryMsg::Info {})
        .unwrap();

    assert_eq!(info, new_info);
}

#[test]
fn to_json() {
    let msg = ExecuteMsg::RegisterVestingAccount {
        address: Addr::unchecked("juno0001".to_string()),
        vesting_amount: Uint128::new(3_000_000_000_000),
        prevesting_amount: Uint128::new(30_000_000_000),
        start_time: Timestamp::from_nanos(1669466438268000000),
        end_time: Timestamp::from_nanos(1669725638268000000),
    };
    println!("{}", serde_json::to_string(&msg).unwrap());
}
