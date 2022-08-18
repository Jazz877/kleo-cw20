use cosmwasm_std::{Empty, Addr, Uint128, coin, Querier, to_binary};
use cw20::{Cw20Coin, Cw20ExecuteMsg};
use cw_multi_test::{Contract, ContractWrapper, App, Executor, next_block};
use crate::{contract, msg::{InstantiateMsg, ExecuteMsg, QueryMsg, VestingAccountResponse}};

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
    );
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
    app.instantiate_contract(vesting_code_id, Addr::unchecked(OWNER), &msg, &[], "cw20", None)
        .unwrap()
}

#[test]
fn cw20_initialization() {
    let mut app = mock_app();
    let cw20_contract_addr = instantiate_cw20(&mut app);
    let vesting_contract_addr = instantiate_vesting(&mut app, cw20_contract_addr.clone());

    println!("{:?}", cw20_contract_addr.clone());
    println!("{:?}", vesting_contract_addr.clone());

    let block_time = app.block_info().time;

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

    // register vesting account
    let _ = app.execute_contract(
        Addr::unchecked(OWNER.to_string()),
        vesting_contract_addr.clone(),
        &ExecuteMsg::RegisterVestingAccount {
            address: Addr::unchecked(USER1.to_string()),
            vesting_amount: Uint128::new(10_000_000),
            start_time: block_time,
            end_time: block_time.plus_seconds(100),
        },
        &vec![],
    );

    // 5seconds more
    app.update_block(next_block);
    
    let msg = QueryMsg::VestingAccount {
        address: Addr::unchecked(USER1.to_string()),
    };

    let res: VestingAccountResponse = app.wrap().query_wasm_smart(vesting_contract_addr.clone(), &msg).unwrap();

    println!("{:?}", res);
    
}