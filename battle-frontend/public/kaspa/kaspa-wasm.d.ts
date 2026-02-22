export default function init(): Promise<void>;
export class RpcClient {
    constructor(config: { url: string; encoding: string });
    connect(): Promise<void>;
    disconnect(): Promise<void>;
}
export class Wallet {
    constructor(config: { networkId: string; mnemonic: string });
    createAccount(name: string): Promise<Account>;
    getAccount(index: number): Promise<Account>;
    addEventListener(event: string, callback: () => void): void;
}
export class Mnemonic {
    constructor(phrase?: string);
    toString(): string;
}
export class Account {
    externalAddress(index: number): string;
    getBalance(): Promise<number>;
    send(params: { destination: string; amount: number }): Promise<{ transactionId: string }>;
}
export enum NetworkId {
    Mainnet = 'mainnet',
    Testnet = 'testnet',
}
