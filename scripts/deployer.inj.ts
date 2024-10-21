import { config } from 'dotenv';
import { Network, getNetworkEndpoints } from '@injectivelabs/networks';
import { MsgStoreCode, MsgInstantiateContract, PrivateKey } from '@injectivelabs/sdk-ts';
import { MsgBroadcasterWithPk } from '@injectivelabs/sdk-ts';
import fs from 'fs';
import path from 'path';
import { DEFAULT_GAS_PRICE } from '@injectivelabs/utils';
import { getINJBalance, getTxInfo } from '../utils/helper';

config();

const { NETWORK, MNEMONIC } = process.env;
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

    if (codeUploadTx.s !== 'ok') return console.log('Unable to retrieve transaction');

    const events = codeUploadTx.data.logs[0].events as any[];
    const attributes: any[] =
        events.find(({ type }) => type === 'cosmwasm.wasm.v1.EventCodeStored')?.attributes || [];
    const code_id = attributes.find(({ key }) => key === 'code_id')?.value;
    if (!code_id) return console.log('Could not get code id');

    const initMsg = {};

    const instantiateMsg = MsgInstantiateContract.fromJSON({
        sender: address,
        admin: address,
        codeId: JSON.parse(code_id),
        label: 'Yoiu Contract',
        msg: initMsg,
    });

    console.log(instantiateMsg);

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

    if (instantiateTx.s !== 'ok')
        return console.log('Unable to retrieve instantiate contract transaction');

    const instantiateEvents = instantiateTx.data.logs[0].events as any[];
    const instantiateAttributes: any[] =
        instantiateEvents.find(({ type }) => type === 'cosmwasm.wasm.v1.EventContractInstantiated')
            ?.attributes || [];
    const contractAddress = instantiateAttributes.find(
        ({ key }) => key === 'contract_address'
    )?.value;
    if (!contractAddress) return console.log('Could not get contract address');

    console.log('Contract Address:', contractAddress);
})();
