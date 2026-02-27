import * as kaspa from 'kaspa-wasm';
import { initKaspaWasm } from './init';
import { getRpcClient } from './rpc';
import { KASPA_NETWORK } from '../config/constants';

export interface WalletConnection {
    wallet: any;
    account: any;
    address: string;
    mnemonic: string;
}


// Wallet aus Mnemonic importieren
export async function importWallet(mnemonicPhrase: string): Promise<WalletConnection> {
    console.log("🔄 Initialisiere Kaspa WASM SDK...");
    await initKaspaWasm();

    console.log("🔌 Verbinde mit Kaspa RPC Node...");
    // Nutze den getRpcClient Aufruf aus rpc.ts für die node url & borsh encoding
    try {
        await getRpcClient();
    } catch (e) {
        throw new Error("Fehler beim Verbinden mit dem Kaspa Netzwerk. Versuche es später erneut.");
    }

    console.log("📥 Importiere und validiere Mnemonic...");

    // 1. Mnemonic validieren
    let mnemonic: kaspa.Mnemonic;
    try {
        mnemonic = new kaspa.Mnemonic(mnemonicPhrase);
    } catch (e) {
        throw new Error("Ungültiger Mnemonic! Bitte prüfe deine 12 oder 24 Wörter (englische Seed-Phrase).");
    }

    console.log("✅ Erstelle Private Key und lade Account 'KaspaBattle'...");

    // 2. Erstelle XPrv (Extended Private Key) aus dem Mnemonic Seed
    const seed = mnemonic.toSeed(""); // Leeres Password
    let xprv: kaspa.XPrv;
    try {
        xprv = new kaspa.XPrv(seed);
    } catch (e) {
        console.error("Fehler beim Erstellen des Master Keys:", e);
        throw new Error("Fehler bei der Key-Derivation (XPrv)!");
    }

    // 3. Verwende PrivateKeyGenerator für BIP44/BIP32 Derivation:
    // "m/44'/111111'/0'" - Account 0, "m/44'/111111'/0'/0/0"
    // is_multisig: false, account_index: 0
    let addressData: { address: kaspa.Address, xpub: string } | null = null;
    let publicKeyGenerator: kaspa.PublicKeyGenerator | null = null;

    try {
        // Erstelle PublicKeyGenerator und PrivateKeyGenerator
        publicKeyGenerator = kaspa.PublicKeyGenerator.fromMasterXPrv(xprv as any, false, 0n);

        // Hole Receive Address 0
        const address = publicKeyGenerator.receiveAddress(KASPA_NETWORK, 0);

        addressData = {
            address: address,
            xpub: xprv.toXPub().xpub, // (Optional) XPub-String exportieren
        };
    } catch (e) {
        console.error("Fehler bei KeyGenerator:", e);
        throw new Error("Fehler beim Ableiten der Empfangsadresse (Derivation)!");
    }

    if (!addressData) {
        throw new Error("Address Derivation fehlgeschlagen");
    }

    // Wandle Objekt in String-Representation mit Prefix `kaspa:` oder `kaspatest:`
    const addressStr = addressData.address.toString();
    console.log(`✅ Adresse erhalten: ${addressStr}`);

    // (Das vorherige sdk hat .Wallet / .createAccount via mnemonic genutzt, dies ist oftmals
    // asymmetrisch in JS Wrappern implementiert, wir gehen nun den direkten BIP32 Derivation Weg.)

    // Stub fÃ¼r Account Object so dass bestehender Code nicht bricht
    const account = {
        externalAddress: addressStr,
        receiveAddress: addressStr,
        xpub: addressData.xpub,
        balance: { mature: 0n, pending: 0n, outgoing: 0n },
        scan: async () => { }, // Mock-Sync
        send: async () => { throw new Error("Send via raw derivation noch nicht voll implementiert!"); },
        getBalance: async () => { return 0; }
    };

    console.log(`✅ Kaspa Wallet (Raw Derivation) erfolgreich geladen! (Adresse: ${addressStr})`);

    // Das alte Wallet Construct ist fÃ¼r Raw Derivation ungenutzt.
    const wallet = null;

    return { wallet, account, address: addressStr, mnemonic: mnemonicPhrase };
}
// Balance abfragen direkt vom Account (verhindert RPC Caching)
export async function getBalance(account: any): Promise<number> {
    try {
        const address = account.externalAddress || account.receiveAddress;
        if (!address) {
            console.warn("⚠️ Keine Adresse im Account gefunden für getBalance");
            return 0;
        }

        const rpcClient = await getRpcClient();
        const res = await rpcClient.getBalanceByAddress({ address });

        // Die Balance wird typischerweise als bigint in Sompi zurückgegeben
        const mature = res.balance;
        return typeof mature === 'bigint' ? Number(mature) : Number(mature || 0);

    } catch (e) {
        console.warn("⚠️ Fehler beim Lesen der Balance", e);
        return 0;
    }
}

// Deposit-TX an Escrow-Adresse senden
export async function sendDeposit(
    account: any,
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
    _wallet: any, // Nicht mehr genutzt im Raw-Derivation-Flow
    account: any,
    callback: (newBalance: number) => void,
): void {
    const address = account.externalAddress || account.receiveAddress;
    if (!address) return;

    // Wir rufen den RpcClient direkt auf, um das UTXO Event zu abonnieren
    getRpcClient().then(rpcClient => {

        // Zuerst einmalig abonnieren
        rpcClient.subscribeUtxosChanged([address]).catch(e => {
            console.error("Konnte UTXOs nicht abonnieren", e);
        });

        (rpcClient as any).addEventListener('utxos-changed', async () => {
            try {
                const newBalance = await getBalance(account);
                callback(newBalance);
            } catch (e) {
                console.error("Fehler beim UTXO Event Callback", e);
            }
        });
    }).catch(console.error);
}
