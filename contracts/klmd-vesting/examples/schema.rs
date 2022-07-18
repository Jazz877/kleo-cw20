use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, remove_schemas, schema_for};
use klmd_vesting::msg::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, MasterAddressResponse, QueryMsg,
    VestingAccountResponse,
};

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("../../klmd-cw3-fixed-multisig/schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(InstantiateMsg), &out_dir);
    export_schema(&schema_for!(ExecuteMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
    export_schema(&schema_for!(Cw20HookMsg), &out_dir);
    export_schema(&schema_for!(VestingAccountResponse), &out_dir);
    export_schema(&schema_for!(MasterAddressResponse), &out_dir);
}
