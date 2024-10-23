import { config } from 'dotenv';
import { Network, getNetworkEndpoints } from '@injectivelabs/networks';
import { MsgStoreCode, MsgInstantiateContract, PrivateKey } from '@injectivelabs/sdk-ts';
import { MsgBroadcasterWithPk } from '@injectivelabs/sdk-ts';
import fs from 'fs';
import path from 'path';
import { DEFAULT_GAS_PRICE } from '@injectivelabs/utils';
import { getINJBalance, getKeyValue, getTxInfo } from '../utils/helper';

config();

const INJ_SWAP_ROUTER = 'inj10x2pnsjlwmdmuzu7klp25hyr222805v4h4tvns';

const { NETWORK, MNEMONIC } = process.env;
// 1. Specify wasm file path
const contractWasmPath = path.join(__dirname, '/../TIER/artifacts/tier.wasm');

(async () => {
    if (!MNEMONIC) throw new Error('MNEMONIC is missing');

    const network = NETWORK && NETWORK === 'mainnet' ? Network.Mainnet : Network.Testnet;
    const endpoints = getNetworkEndpoints(network);

    const pk = PrivateKey.fromMnemonic(MNEMONIC);
    const address = pk.toBech32();

    console.log('Address: ', address);

    console.log('\nFetching balance........');
    const balance = await getINJBalance(address, endpoints);
    console.log(`Balance: ${balance} inj`);

    const broadcaster = new MsgBroadcasterWithPk({
        endpoints,
        network,
        privateKey: pk.toPrivateKeyHex(),
    });

    const wasmBytes = fs.readFileSync(contractWasmPath);
    const codeUploadMsg = MsgStoreCode.fromJSON({ sender: address, wasmBytes });

    console.log('\nSimulating transaction........');
    const codeUploadSimulationResponse = await broadcaster.simulate({
        msgs: codeUploadMsg,
    });

    const codeUploadGasOptions = {
        feePayer: address,
        gas: Math.ceil(Number(codeUploadSimulationResponse.gasInfo.gasUsed) * 1.3),
        gasPrice: DEFAULT_GAS_PRICE.toString(),
    };

    console.log('Broadcasting transaction........');
    const codeUploadBroadcastResponse = await broadcaster.broadcast({
        msgs: codeUploadMsg,
        gas: codeUploadGasOptions,
    });

    console.log('\nGetting transaction info........');
    const codeUploadTx = await getTxInfo(
        codeUploadBroadcastResponse.txHash,
        NETWORK === 'mainnet' ? undefined : 'testnet'
    );

    const code_id = getKeyValue(codeUploadTx, 'cosmwasm.wasm.v1.EventCodeStored', 'code_id');
    console.log('Code ID: ', code_id);

    // 2. Provide inistantiate message
    // For TIER
    const initMsg = {
        validators: [
            {
                address: 'injvaloper1cq6mvxqp978f6lxrh5s6c35ddr2slcj9h7tqng',
                weight: '100',
            },
        ],
        oraiswap_contract: {
            usdt_contract: 'peggy0x87aB3B4C8661e07D6372361211B96ed4Dc36B1B5',
            orai_swap_router_contract: INJ_SWAP_ROUTER,
        },
        deposits: ['25000', '7500', '1500', '250'],
        admin: address,
    };

    // FOR IDO
    // const initMsg = {
    //     lock_periods: [864000, 1209600, 1209600, 1209600, 1209600],
    //     nft_contract: 'inj19ly43dgrr2vce8h02a8nw0qujwhrzm9yv8d75c',
    //     tier_contract: 'inj15nmkxpn9a4lfd5e555ggeldte0zlqmm9695h77',
    // };

    const instantiateMsg = MsgInstantiateContract.fromJSON({
        sender: address,
        admin: address,
        codeId: Number(JSON.parse(code_id)),
        label: 'Yoiu Contract',
        msg: initMsg,
    });

    console.log('\nSimulating instantiate contract transaction........');
    const instantiateSimulationResponse = await broadcaster.simulate({
        msgs: instantiateMsg,
    });

    const instantiateGasOptions = {
        feePayer: address,
        gas: Math.ceil(Number(instantiateSimulationResponse.gasInfo.gasUsed) * 1.3),
        gasPrice: DEFAULT_GAS_PRICE.toString(),
    };

    console.log('Broadcasting instantiate contract transaction........');
    const instantiateResponse = await broadcaster.broadcast({
        msgs: instantiateMsg,
        gas: instantiateGasOptions,
    });

    console.log('\nGetting instantiate contract transaction info........');
    const instantiateTx = await getTxInfo(
        instantiateResponse.txHash,
        NETWORK === 'mainnet' ? undefined : 'testnet'
    );

    const contractAddress = getKeyValue(
        instantiateTx,
        'cosmwasm.wasm.v1.EventContractInstantiated',
        'contract_address'
    );

    console.log('Contract Address:', contractAddress);
})();
