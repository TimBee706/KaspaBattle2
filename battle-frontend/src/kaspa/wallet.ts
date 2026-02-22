import { Wallet, Mnemonic, NetworkId, Account } from 'kaspa-wasm';
import { initKaspaWasm } from './init';
import { getRpcClient } from './rpc';

export interface WalletConnection {
    wallet: Wallet;
    account: Account;
    address: string;
}

// Neues Wallet erstellen (Registrierung / Erstanmeldung)
export async function createNewWallet(): Promise<{ connection: WalletConnection; mnemonic: string }> {
    await initKaspaWasm();
    const mnemonic = new Mnemonic();          // 12 Wörter generieren
    const phraseString = mnemonic.toString();

    const wallet = new Wallet({
        networkId: NetworkId.Mainnet as any,
        mnemonic: phraseString,
    });

    const account = await wallet.createAccount('KaspaBattle');
    const address = account.externalAddress(0);

    return {
        connection: { wallet, account, address },
        mnemonic: phraseString,
    };
}

// Wallet aus Mnemonic importieren
export async function importWallet(mnemonic: string): Promise<WalletConnection> {
    await initKaspaWasm();
    const wallet = new Wallet({
        networkId: NetworkId.Mainnet as any,
        mnemonic: mnemonic,
    });

    const account = await wallet.createAccount('KaspaBattle');
    const address = account.externalAddress(0);

    return { wallet, account, address };
}

// Balance abfragen (in Sompi)
export async function getBalance(address: string): Promise<number> {
    const rpc = await getRpcClient();
    const response = await (rpc as any).getBalanceByAddress(address);
    return response.balance;  // in Sompi
}

// Deposit-TX an Escrow-Adresse senden
export async function sendDeposit(
    account: Account,
    escrowAddress: string,
    amountSompi: number,
): Promise<string> {
    const txResult = await (account as any).send({
        destination: escrowAddress,
        amount: amountSompi,
    });
    return txResult.transactionId;
}

// Event-Listener für Balance-Änderungen
export function onBalanceChange(
    wallet: Wallet,
    callback: (newBalance: number) => void,
): void {
    (wallet as any).addEventListener('utxoChanged', async () => {
        const account = await (wallet as any).getAccount(0);
        const balance = await account.getBalance();
        callback(balance);
    });
}
