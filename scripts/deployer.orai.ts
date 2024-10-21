import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing';
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate';
import { GasPrice } from '@cosmjs/stargate';
import fs from 'fs';
import path from 'path';
import dotenv from 'dotenv';

dotenv.config();

const mnemonic = process.env.MNEMONIC;
const rpcEndpoint = 'https://testnet.rpc.orai.io';
const contractWasmPath = path.join(__dirname, '/../IDO/artifacts/ido.wasm');

async function deploy() {
    if (!mnemonic) throw new Error('MNEMONIC is missing');

    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: 'orai' });
    const [account] = await wallet.getAccounts();
    console.log(`Wallet address: ${account.address}`);

    const client = await SigningCosmWasmClient.connectWithSigner(rpcEndpoint, wallet, {
        gasPrice: GasPrice.fromString('0.0025orai'),
    });
    console.log('Connected to blockchain');

    const wasmCode = fs.readFileSync(contractWasmPath);
    const balance = await client.getBalance(account.address, 'orai');
    console.log(`Balance: ${Number(balance.amount) / 1_000_000} ${balance.denom}`);

    const uploadReceipt = await client.upload(
        account.address,
        wasmCode,
        'auto',
        'Upload CosmWasm contract'
    );

    const codeId = uploadReceipt.codeId;
    console.log(`Contract uploaded with Code ID: ${codeId}`);

    const initMsg = {};

    const instantiateReceipt = await client.instantiate(
        account.address,
        codeId,
        initMsg,
        'Instantiate Contract',
        'auto'
    );

    const contractAddress = instantiateReceipt.contractAddress;
    console.log(`Contract instantiated at reciept: ${instantiateReceipt}`);
    console.log(`Contract instantiated at address: ${contractAddress}`);
}

deploy().catch((e) => console.error(e));
