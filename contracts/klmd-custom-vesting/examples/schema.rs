use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, remove_schemas, schema_for};

use klmd_custom_vesting::msg::{ExecuteMsg, InstantiateMsg, OwnerAddressResponse, QueryMsg, TokenAddressResponse, VestingAccountResponse};

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(InstantiateMsg), &out_dir);
    export_schema(&schema_for!(ExecuteMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
    export_schema(&schema_for!(OwnerAddressResponse), &out_dir);
    export_schema(&schema_for!(VestingAccountResponse), &out_dir);
    export_schema(&schema_for!(TokenAddressResponse), &out_dir);
}
