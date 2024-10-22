import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing';
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate';
import { GasPrice } from '@cosmjs/stargate';
import fs from 'fs';
import path from 'path';
import dotenv from 'dotenv';

dotenv.config();

const USDT_CONTRACT = 'orai1laj3d4zledty0r0vd7m3gem4cd7cyk09m608863p3t7p6sm6xmusru4l5p';
const ORAI_SWAP_ROUTER_CONTRACT = 'orai1e4k9zhjpz3a0q6fgspwj3ug5fhc3t2emtxuvtra79hs70gqq7m0sg8kdd5';

const mnemonic = process.env.MNEMONIC;
const rpcEndpoint = 'https://testnet.rpc.orai.io';
const contractWasmPath = path.join(__dirname, '/../TIER/artifacts/tier.wasm');

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

    const initMsg = {
        validators: [
            {
                address: 'oraivaloper18hr8jggl3xnrutfujy2jwpeu0l76azprkxn29v',
                weight: '100',
            },
        ],
        oraiswap_contract: {
            usdt_contract: USDT_CONTRACT,
            orai_swap_router_contract: ORAI_SWAP_ROUTER_CONTRACT,
        },
        deposits: ['25000', '7500', '1500', '250'],
        admin: account.address,
    };

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
