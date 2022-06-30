import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { coins, DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { assertIsDeliverTxSuccess, calculateFee, GasPrice } from "@cosmjs/stargate";
import sha256 from 'crypto-js/sha256'
import { MerkleTree } from 'merkletreejs';
import * as fs from "fs";

const localUser = {
    address: "juno1gz2c6rkwf9quztvwynwr2hsa9lmldnjlk6qy75",
    mnemonic: "ranch uphold warm club ribbon hamster battle master conduct era lemon amazing pledge glue sniff coconut record shove stamp morning august cluster rack black",
};
const localClaimer = {
    address: "juno1s6ehd39jel0tjfpkpkqrf3dzkszkg0pandxdcn",
    mnemonic: "check response pitch fatigue trumpet main upper until entire kiss business curtain output picture portion heavy unfold maple summer pet game seven super they"
}
const gasPrice = GasPrice.fromString("0.0025ujunox");
const rpcEndpoint = "https://rpc.uni.juno.deuslabs.fi:443";

async function uploadCw20(cw20WasmPath: string) : Promise<number> {
    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(localUser.mnemonic, { prefix: "juno" });
    const [{ address: signerAddress }] = await wallet.getAccounts();
    console.log("Signer address:", signerAddress);

    // Upload contract
    const client = await SigningCosmWasmClient.connectWithSigner(rpcEndpoint, wallet, {"gasPrice": gasPrice});
    
    const wasm = fs.readFileSync(cw20WasmPath);
    const uploadFee = calculateFee(29_099_252, gasPrice);
    const uploadReceipt = await client.upload(signerAddress, wasm, 'auto', "KLMD upload");
    console.log("Upload succeeded. Receipt:", uploadReceipt);

    return uploadReceipt.codeId;
}

async function instantiateCw20(codeId: number, initialSupply: number = 64000000000000) : Promise<string> {
    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(localUser.mnemonic, { prefix: "juno" });
    const [{ address: signerAddress }] = await wallet.getAccounts();
    const instantiateFee = calculateFee(500_000, gasPrice);
    const client = await SigningCosmWasmClient.connectWithSigner(rpcEndpoint, wallet);
    const instantiateMsg = {
        "name":"Kleomedes",
        "symbol":"KLMD",
        "decimals":6,
        "initial_balances":[{"address": signerAddress,"amount": initialSupply.toString()}]
    };
    const { contractAddress } = await client.instantiate(
        signerAddress,
        codeId,
        instantiateMsg,
        "KLMD Instance",
        instantiateFee,
        { memo: `Create a KLMD Instance CW20` },
      );
      console.info(`Contract instantiated at: `, contractAddress);
      return contractAddress;
}

async function queryBalance(contractAddress: string) {
    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(localUser.mnemonic, { prefix: "juno" });
    const [{ address: signerAddress }] = await wallet.getAccounts();
    const client = await SigningCosmWasmClient.connectWithSigner(rpcEndpoint, wallet);

    const balanceMsg = {
        "balance":{"address": signerAddress}
    };
    const queryResult = await client.queryContractSmart(contractAddress, balanceMsg);
    console.log("Balance ", queryResult);
}

async function uploadAirdrop(airdropWasmPath: string) {
    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(localUser.mnemonic, { prefix: "juno" });
    const [{ address: signerAddress }] = await wallet.getAccounts();
    console.log("Signer address:", signerAddress);

    // Upload contract
    const client = await SigningCosmWasmClient.connectWithSigner(rpcEndpoint, wallet, {"gasPrice": gasPrice});
    
    const wasm = fs.readFileSync(airdropWasmPath);
    const uploadFee = calculateFee(29_099_252, gasPrice);
    const uploadReceipt = await client.upload(signerAddress, wasm, 'auto', "KLMD upload");
    console.log("Upload succeeded. Receipt:", uploadReceipt);

    return uploadReceipt.codeId;
}

async function instantiateAirdrop(codeId: number, cw20ContractAddr: string) : Promise<string> {
    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(localUser.mnemonic, { prefix: "juno" });
    const [{ address: signerAddress }] = await wallet.getAccounts();
    const instantiateFee = calculateFee(500_000, gasPrice);
    const client = await SigningCosmWasmClient.connectWithSigner(rpcEndpoint, wallet, {"gasPrice": gasPrice});
    const instantiateMsg = {"cw20_token_address": cw20ContractAddr};
    const { contractAddress } = await client.instantiate(
        signerAddress,
        codeId,
        instantiateMsg,
        "KLMD Airdrop Instance",
        'auto',
        { memo: `Create a Airdrop Instance` },
      );
      console.info(`Contract instantiated at: `, contractAddress);
      return contractAddress;
}

function getMerkleRoot(airdropJsonPath: string) : string {
    try {
        const file = fs.readFileSync(airdropJsonPath, 'utf-8');
        const receivers: Array<{ address: string; amount: string }> = JSON.parse(file);
        const leaves = receivers.map((a) => sha256(a.address + a.amount));
        const tree = new MerkleTree(leaves, sha256, { sort: true });
        return tree.getHexRoot().replace('0x', '');
    } catch (e) {
        console.error(e);
        return "";
    }
    
}

async function addMerkleRootToAirdrop(airdropContractAddr: string, merkleRoot: string) {
    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(localUser.mnemonic, { prefix: "juno" });
    const [{ address: signerAddress }] = await wallet.getAccounts();

    const executeFee = calculateFee(118_376, gasPrice);
    const client = await SigningCosmWasmClient.connectWithSigner(rpcEndpoint, wallet, {"gasPrice": gasPrice});

    const registerMsg = {"register_merkle_root": { "merkle_root": merkleRoot}};
    const result = await client.execute(signerAddress, airdropContractAddr, registerMsg, 'auto', "KLMD Airdrop add merkle root");
    console.info(`Register merkle: `, result);
}

async function queryRegisteredMerkleRoot(airdropContractAddr: string, stage: number) : Promise<string> {
    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(localUser.mnemonic, { prefix: "juno" });
    const [{ address: signerAddress }] = await wallet.getAccounts();
    const client = await SigningCosmWasmClient.connectWithSigner(rpcEndpoint, wallet, {"gasPrice": gasPrice});

    const queryMsg = {
        "merkle_root": {"stage": stage}
    };
    const response = await client.queryContractSmart(airdropContractAddr, queryMsg);
    console.log(response);
    return response.merkle_root;
}

function getMerkleProof(airdropJsonPath: string, addr: string, amount: number) : Array<string> {
    try {
        const file = fs.readFileSync(airdropJsonPath, 'utf-8');
        const receivers: Array<{ address: string; amount: string }> = JSON.parse(file);
        const leaves = receivers.map((a) => sha256(a.address + a.amount));
        const tree = new MerkleTree(leaves, sha256, { sort: true });
        const proofs = tree.getHexProof(sha256(addr + amount.toString()).toString()).map((v) => v.replace('0x', ''));
        console.log("Merkle proof: ", proofs);
        return proofs;
    } catch (e) {
        console.error(e);
        return [];
    }
}

async function claimAirdrop(userProof: Array<string>, addr: string, amount: number, stage: number, airdropContractAddr: string) {
    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(localClaimer.mnemonic, { prefix: "juno" });
    const [{ address: signerAddress }] = await wallet.getAccounts();
    const executeFee = calculateFee(190_324, gasPrice);
    const client = await SigningCosmWasmClient.connectWithSigner(rpcEndpoint, wallet, {"gasPrice": gasPrice});

    const claimMsg = {"claim": { "amount": amount.toString(), "proof": userProof, "stage": stage}};
    const result = await client.execute(signerAddress, airdropContractAddr, claimMsg, 'auto', "claim");
    console.log(result);
}

/*
async function transferUCosm(receiver: string) {
    const gasPrice = GasPrice.fromString("0.025ucosm");
    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(localClaimer.mnemonic, { prefix: "juno" });
    const [{ address: signerAddress }] = await wallet.getAccounts();
    const client = await SigningCosmWasmClient.connectWithSigner(rpcEndpoint, wallet, { gasPrice: gasPrice });
    const recipient = receiver;
    const amount = coins(100, "ucosm");
    const memo = "With simulate";
    const result = await client.sendTokens(signerAddress, recipient, amount, "auto", memo);
    assertIsDeliverTxSuccess(result);
}
*/

async function queryBalanceUser(contractAddress: string, receiver: string) {
    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(localUser.mnemonic, { prefix: "juno" });
    const [{ address: signerAddress }] = await wallet.getAccounts();
    const client = await SigningCosmWasmClient.connectWithSigner(rpcEndpoint, wallet);

    const balanceMsg = {
        "balance":{"address": receiver}
    };
    const queryResult = await client.queryContractSmart(contractAddress, balanceMsg);
    console.log("Balance ", queryResult);
}

async function transferFundsToAirdrop(cw20ContractAddr: string, airdropContractAddr: string, amount: number) {
    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(localUser.mnemonic, { prefix: "juno" });
    const [{ address: signerAddress }] = await wallet.getAccounts();
    const client = await SigningCosmWasmClient.connectWithSigner(rpcEndpoint, wallet, {"gasPrice": gasPrice});
    const executeFee = calculateFee(179_263, gasPrice);
    const transferMsg = {
        "transfer": {
            "amount": amount.toString(),
            "recipient": airdropContractAddr
        }
    };
    const result = await client.execute(signerAddress, cw20ContractAddr, transferMsg, 'auto', "Airdrop transfer");
    console.log("Fund transferd to airdrop", result);
}

(async () => {
    try {
        // remember to run 'junod tx bank send localsigner juno1s6ehd39jel0tjfpkpkqrf3dzkszkg0pandxdcn 100000ucosm --chain-id testing'
        //const cw20CodeId = await uploadCw20("../artifacts/klmd_cw20.wasm");
        const klmdContractAddr = "juno196uzuuquk9jj8gjep3unts2mp5slj04p3m8t0kh762judhdqrrqqt9tkgj";//await instantiateCw20(cw20CodeId);
        await queryBalance(klmdContractAddr);
        //const airdropCodeId = await uploadAirdrop("../artifacts/cw20_merkle_airdrop.wasm");
        const airdropContractAddr = "juno14yl52e6a830hyc6w752ugta023hdr5ev4k6x67rxj3hftfzf9tzqzjvd60"; //await instantiateAirdrop(airdropCodeId, klmdContractAddr);
        let merkleRoot = getMerkleRoot("airdrop_stage1_testnet_list.json");
        console.log("Merkle root: ", merkleRoot);
        await addMerkleRootToAirdrop(airdropContractAddr, merkleRoot);
        merkleRoot = await queryRegisteredMerkleRoot(airdropContractAddr, 1);
        //await transferFundsToAirdrop(klmdContractAddr, airdropContractAddr, 1000);
        //await queryBalance(klmdContractAddr);
        //const userProof = await getMerkleProof("airdrop_stage_list.json", localClaimer.address, 400);
        //await claimAirdrop(userProof, localClaimer.address, 400, 1, airdropContractAddr);
        //await queryBalanceUser(klmdContractAddr, localClaimer.address);
    } catch (e) {
        console.error(e);
    }
})();