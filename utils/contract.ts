import { Network, NetworkEndpoints, getNetworkEndpoints } from '@injectivelabs/networks';
import {
    ChainGrpcWasmApi,
    MsgBroadcasterWithPk,
    MsgExecuteContractCompat,
    PrivateKey,
    fromBase64,
    toBase64,
} from '@injectivelabs/sdk-ts';
import { DEFAULT_GAS_PRICE } from '@injectivelabs/utils';
import { config } from 'dotenv';

config();

class ContractSdk {
    endpoints: NetworkEndpoints;
    broadcaster: MsgBroadcasterWithPk;
    address: string;
    chainGrpcWasmApi: ChainGrpcWasmApi;

    constructor(mnemonic: string, private network: Network = Network.Testnet) {
        this.endpoints = getNetworkEndpoints(this.network);
        this.chainGrpcWasmApi = new ChainGrpcWasmApi(this.endpoints.grpc);

        const pk = PrivateKey.fromMnemonic(mnemonic);

        this.address = pk.toBech32();
        this.broadcaster = new MsgBroadcasterWithPk({
            endpoints: this.endpoints,
            network: this.network,
            privateKey: pk.toPrivateKeyHex(),
        });
    }

    async query(contractAddress: string, query: object) {
        try {
            const response = await this.chainGrpcWasmApi.fetchSmartContractState(
                contractAddress,
                toBase64(query)
            );

            return fromBase64(response.data as any) as any;
        } catch (error) {
            console.error('Error querying contract:', error);
            throw error;
        }
    }

    async execute(
        contractAddress: string,
        msg: object,
        funds?: { amount: string; denom: string }[]
    ) {
        const message = MsgExecuteContractCompat.fromJSON({
            sender: this.address,
            contractAddress: contractAddress,
            msg: msg,
            funds: funds ? funds : [],
        });

        const { gasInfo } = await this.broadcaster.simulate({ msgs: message });

        return await this.broadcaster.broadcast({
            msgs: message,
            gas: {
                gas: Math.ceil(Number(gasInfo.gasUsed) * 1.3),
                gasPrice: DEFAULT_GAS_PRICE.toString(),
            },
        });
    }
}

const { NETWORK, MNEMONIC } = process.env;
if (!MNEMONIC) throw new Error('MNEMONIC is missing');

const contractSdk = new ContractSdk(
    MNEMONIC,
    NETWORK && NETWORK === 'mainnet' ? Network.Mainnet : Network.Testnet
);

export default contractSdk;
