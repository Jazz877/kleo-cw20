TEST_VALIDATOR_ADDR = juno16g2rahf5846rxzp3fwlswy08fz8ccuwk03k57y
LOCAL_KEY = local

setup:
	rustup target add wasm32-unknown-unknown

juno:
	git clone https://github.com/CosmosContracts/juno.git
	cd juno && git fetch && git checkout v2.1.0 && make install

optimize:
	docker run --rm -v "$(shell pwd)":/code \
	--mount type=volume,source="$(shell basename "$(shell pwd)")_cache",target=/code/target \
	--mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
	cosmwasm/workspace-optimizer:0.12.5

add-local-wallet:
	junod keys add ${LOCAL_KEY} --recover

store-klmd:
	cd artifacts && junod tx wasm store ${CTR_NAME}.wasm  --from ${LOCAL_KEY} --chain-id=testing --gas 25515150 --output json -y | jq -r '.txhash'

get-code-id:
	junod query tx ${TX} --output json | jq -r '.logs[0].events[-1].attributes[0].value'

init-cw20-contract:
	@ junod tx wasm instantiate ${CODE_ID} \
    '{"name":"Kleomedes","symbol":"KLMD","decimals":6,"initial_balances":[{"address":"${TEST_VALIDATOR_ADDR}","amount":"64000000000000"}]}' \
    --amount 0ujunox  --label "Kleomedes cw20" --from local --chain-id testing --gas 149949 -y

get-contract-address:
	@ junod query wasm list-contract-by-code ${CODE_ID} --output json

get-balance:
	@ junod query wasm contract-state smart ${CTR_ADDR} '{"balance":{"address":"${TEST_VALIDATOR_ADDR}"}}'

init-airdrop-contract:
	@ junod tx wasm instantiate ${CODE_ID} \
    '{"cw20_token_address": "${CW20_CTR_ADDR}"}' \
    --amount 0ujunox  --label "Kleomedes cw20" --from ${LOCAL_KEY} --chain-id testing --gas 149949 -y

register-merkle-root:
	@ junod tx wasm execute ${AIRDROP_CTR_ADDR} \
	'{"register_merkle_root": { "merkle_root": "${MERKLE_ROOT}"}}' \
	--from ${LOCAL_KEY} --chain-id testing --gas 117376 -y

claim-with-proof:
	@ junod tx wasm execute ${AIRDROP_CTR_ADDR} \
	'{"claim": {"amount": "${AMOUNT}", "proof": "${PROOF}", "stage": "${STAGE}"}}' \
	--from ${LOCAL_KEY} --chain-id testing --gas auto -y