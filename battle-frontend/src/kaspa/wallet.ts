import * as kaspa from 'kaspa-wasm';
import { initKaspaWasm } from './init';
import { getRpcClient } from './rpc';
import { KASPA_NETWORK } from '../config/constants';
import { getErrorMessage } from '../utils/errors';

export { getRpcClient };

export interface SafeAccount {
    receiveAddress: string;
    escrowAddress: string;
    xpub: string;
    publicKey: string;
    mnemonicPhrase?: string;
}

export interface WalletConnection {
    wallet: unknown;
    account: SafeAccount;
    address: string;
}

export interface ImportResult {
    connection: WalletConnection;
    ephemeralPrivateKeyHex: string;
}

type RpcClientWithEvents = Awaited<ReturnType<typeof getRpcClient>> & {
    addEventListener?: (event: 'utxos-changed', listener: () => void) => void;
};

type MasterXPrvFactory = {
    fromMasterXPrv(xprv: kaspa.XPrv, account: boolean, index: bigint): kaspa.PublicKeyGenerator;
};

function getMasterXPrvFactory(): MasterXPrvFactory {
    return kaspa.PublicKeyGenerator as unknown as MasterXPrvFactory;
}

export async function importWallet(mnemonicPhrase: string): Promise<ImportResult> {
    await initKaspaWasm();

    try {
        await getRpcClient();
    } catch {
        throw new Error('Fehler beim Verbinden mit dem Kaspa Netzwerk. Versuche es spaeter erneut.');
    }

    let mnemonic: kaspa.Mnemonic;
    try {
        mnemonic = new kaspa.Mnemonic(mnemonicPhrase);
    } catch {
        throw new Error('Ungueltiger Mnemonic! Bitte pruefe deine 12 oder 24 Woerter.');
    }

    const seed = mnemonic.toSeed('');
    const xprv = new kaspa.XPrv(seed);
    const publicKeyGenerator = getMasterXPrvFactory().fromMasterXPrv(xprv, false, 0n);
    const privateKeyGenerator = new kaspa.PrivateKeyGenerator(xprv, false, 0n);

    const address = publicKeyGenerator.receiveAddress(KASPA_NETWORK, 0).toString();
    const escrowAddress = publicKeyGenerator.receiveAddress(KASPA_NETWORK, 1).toString();
    const privateKey = privateKeyGenerator.receiveKey(0);
    const publicKey = privateKey.toPublicKey().toString();
    const ephemeralPrivateKeyHex = privateKey.toString();

    const account: SafeAccount = {
        receiveAddress: address,
        escrowAddress,
        xpub: xprv.toXPub().xpub,
        publicKey,
        mnemonicPhrase,
    };

    return {
        connection: { wallet: null, account, address },
        ephemeralPrivateKeyHex,
    };
}

export async function getBalance(xpub: string): Promise<number> {
    try {
        if (!xpub) return 0;

        await initKaspaWasm();
        const rpcClient = await getRpcClient();
        const publicKeyGenerator = kaspa.PublicKeyGenerator.fromXPub(xpub, false);

        let totalSompi = 0n;
        const addressesToScan: string[] = [];

        for (let i = 0; i < 20; i++) {
            addressesToScan.push(publicKeyGenerator.receiveAddress(KASPA_NETWORK, i).toString());
        }
        for (let i = 0; i < 20; i++) {
            addressesToScan.push(publicKeyGenerator.changeAddress(KASPA_NETWORK, i).toString());
        }

        for (const addr of addressesToScan) {
            try {
                const res = await rpcClient.getBalanceByAddress({ address: addr });
                if (res.balance) {
                    totalSompi += typeof res.balance === 'bigint' ? res.balance : BigInt(res.balance || 0);
                }
            } catch {
                // Ignore empty/invalid address lookups during scan.
            }
        }

        return Number(totalSompi);
    } catch (e) {
        console.warn('Fehler beim Scannen der Balances', e);
        return 0;
    }
}

export async function getBalanceByAddress(address: string): Promise<number> {
    try {
        const rpcClient = await getRpcClient();
        const res = await rpcClient.getBalanceByAddress({ address });
        if (res.balance) {
            return Number(typeof res.balance === 'bigint' ? res.balance : BigInt(res.balance || 0));
        }
        return 0;
    } catch (e) {
        console.error(`[wallet] Error fetching balance for ${address}:`, e);
        throw e;
    }
}

export async function sendDeposit(
    mnemonicPhrase: string,
    escrowAddress: string,
    amountSompi: number,
): Promise<string> {
    if (!mnemonicPhrase) throw new Error('Mnemonic fehlt');

    await initKaspaWasm();
    const rpc = await getRpcClient();

    const mnemonic = new kaspa.Mnemonic(mnemonicPhrase);
    const seed = mnemonic.toSeed('');
    const xprv = new kaspa.XPrv(seed);
    const privateKeyGenerator = new kaspa.PrivateKeyGenerator(xprv, false, 0n);
    const publicKeyGenerator = getMasterXPrvFactory().fromMasterXPrv(xprv, false, 0n);

    const addressesToScan: string[] = [];
    const keyMap = new Map<string, kaspa.PrivateKey>();

    for (let i = 0; i < 20; i++) {
        const receiveAddress = publicKeyGenerator.receiveAddress(KASPA_NETWORK, i).toString();
        addressesToScan.push(receiveAddress);
        keyMap.set(receiveAddress, privateKeyGenerator.receiveKey(i));

        const changeAddress = publicKeyGenerator.changeAddress(KASPA_NETWORK, i).toString();
        addressesToScan.push(changeAddress);
        keyMap.set(changeAddress, privateKeyGenerator.changeKey(i));
    }

    const { entries } = await rpc.getUtxosByAddresses(addressesToScan);
    if (!entries || entries.length === 0) {
        throw new Error('Kein Guthaben (UTXOs) auf dem Wallet gefunden. Bitte lade dein Wallet auf.');
    }

    const usedPrivateKeys: kaspa.PrivateKey[] = [];
    const usedKeyStrings = new Set<string>();

    for (const entry of entries) {
        const addr = entry.address?.toString();
        if (addr && keyMap.has(addr)) {
            const pk = keyMap.get(addr);
            if (!pk) continue;
            const pkStr = pk.toString();
            if (!usedKeyStrings.has(pkStr)) {
                usedKeyStrings.add(pkStr);
                usedPrivateKeys.push(pk);
            }
        }
    }

    const amount = BigInt(amountSompi);
    const changeAddress = publicKeyGenerator.receiveAddress(KASPA_NETWORK, 0).toString();

    let transactions: Awaited<ReturnType<typeof kaspa.createTransactions>>['transactions'];
    try {
        const result = await kaspa.createTransactions({
            entries,
            outputs: [{ address: escrowAddress, amount }],
            priorityFee: 0n,
            changeAddress,
            networkId: KASPA_NETWORK,
        });
        transactions = result.transactions;
    } catch (e) {
        throw new Error(`Transaktionserstellung fehlgeschlagen: ${getErrorMessage(e, 'Unbekannter WASM Fehler')}`);
    }

    if (!transactions || transactions.length === 0) {
        throw new Error('Transaktionserstellung fehlgeschlagen: Keine Transaktionen generiert.');
    }

    let finalTxId = '';
    for (const pending of transactions) {
        try {
            await pending.sign(usedPrivateKeys);
        } catch (e) {
            throw new Error(`Signatur-Fehler: ${getErrorMessage(e)}`);
        }

        try {
            finalTxId = await pending.submit(rpc);
        } catch (e) {
            throw new Error(`RPC-Submit-Fehler: ${getErrorMessage(e)}`);
        }
    }

    return finalTxId;
}

export function onBalanceChange(
    _wallet: unknown,
    address: string,
    callback: (newBalance: number) => void,
): void {
    if (!address) return;

    getRpcClient().then((rpcClient) => {
        rpcClient.subscribeUtxosChanged([address]).catch((e) => {
            console.error('Konnte UTXOs nicht abonnieren', e);
        });

        (rpcClient as RpcClientWithEvents).addEventListener?.('utxos-changed', async () => {
            try {
                const newBalance = await getBalanceByAddress(address);
                callback(newBalance);
            } catch (e) {
                console.error('Fehler beim UTXO Event Callback', e);
            }
        });
    }).catch(console.error);
}
