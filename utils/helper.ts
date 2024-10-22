import { ChainGrpcBankApi } from '@injectivelabs/sdk-ts';
import { BigNumberInBase } from '@injectivelabs/utils';

export async function getINJBalance(address: string, endpoints: any) {
    const bankApi = new ChainGrpcBankApi(endpoints.grpc);
    const balances = await bankApi.fetchBalances(address);

    const injBalance = balances.balances.find((balance) => balance.denom === 'inj');

    if (!injBalance) return '0';
    return (new BigNumberInBase(injBalance.amount).toNumber() / 1_000_000_000_000_000_000).toFixed(
        2
    );
}

export async function getTxInfo(hash: string, network: 'testnet' | undefined = 'testnet') {
    const url = `https://${network}.sentry.exchange.grpc-web.injective.network/api/explorer/v1/txs/${hash}`;

    const response = await fetch(url, {
        method: 'GET',
        headers: { 'Content-Type': 'application/json' },
    });

    return await response.json();
}

export function getKeyValue(tx: any, messageType: string, key: string) {
    if (tx.s !== 'ok') throw Error('Unable to retrieve instantiate contract transaction');

    const events = tx.data.logs[0].events as any[];
    const attributes: any[] = events.find(({ type }) => type === messageType)?.attributes || [];

    const value = attributes.find(({ key: objKey }) => objKey === key)?.value;
    if (!value) throw Error(`Could not retrieve value for: ${key}`);

    return value;
}
