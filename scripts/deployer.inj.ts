import { config } from 'dotenv';
import { Network, getNetworkEndpoints } from '@injectivelabs/networks';
import { MsgStoreCode, MsgInstantiateContract, PrivateKey } from '@injectivelabs/sdk-ts';
import { MsgBroadcasterWithPk } from '@injectivelabs/sdk-ts';
import fs from 'fs';
import path from 'path';
import { DEFAULT_GAS_PRICE } from '@injectivelabs/utils';
import { getINJBalance, getKeyValue, getTxInfo } from '../utils/helper';

config();

const { NETWORK, MNEMONIC } = process.env;
// 1. Specify wasm file path
const contractWasmPath = path.join(__dirname, '/../IDO/artifacts/ido.wasm');

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

    console.log(codeUploadMsg.toAmino().type);

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

    // 2. Provide inistantiate message
    const initMsg = {
        lock_periods: ['864000', '1209600', '1209600', '1209600', '1209600'],
        nft_contract: address, // FIXME: not valid pls, just to test contract instantiation
        tier_contract: address, // FIXME: not valid pls, just to test contract instantiation
    };

    const instantiateMsg = MsgInstantiateContract.fromJSON({
        sender: address,
        admin: address,
        codeId: Number(JSON.parse(code_id)),
        label: 'Yoiu Contract',
        msg: initMsg,
    });

    console.log(instantiateMsg.toData());
    console.log(instantiateMsg.toAmino().type);

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
