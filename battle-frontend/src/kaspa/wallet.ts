import * as kaspa from 'kaspa-wasm';
import { initKaspaWasm } from './init';
import { getRpcClient } from './rpc';
export { getRpcClient };
import { KASPA_NETWORK } from '../config/constants';

// Wallet Connection interface
export interface WalletConnection {
    wallet: any;
    account: {
        receiveAddress: string;
        escrowAddress: string;
        xpub: string;
        mnemonic: string;
        privateKeyHex: string;
        publicKey: string;
    };
    address: string;
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
    let privateKeyGenerator: kaspa.PrivateKeyGenerator | null = null;

    try {
        // Erstelle PublicKeyGenerator und PrivateKeyGenerator
        publicKeyGenerator = kaspa.PublicKeyGenerator.fromMasterXPrv(xprv as any, false, 0n);
        privateKeyGenerator = new kaspa.PrivateKeyGenerator(xprv, false, 0n);

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

    // Generiere Escrow-Adresse (Index 1) – eine valide Testnet-Adresse mit korrekter Checksumme
    const escrowAddress = publicKeyGenerator!.receiveAddress(KASPA_NETWORK, 1).toString();
    console.log(`🔐 Escrow-Adresse generiert (Index 1): ${escrowAddress}`);
    const privateKey = privateKeyGenerator!.receiveKey(0);
    const publicKey = privateKey.toPublicKey().toString();
    const privateKeyHex = privateKey.toString();

    // Stub für Account Object so dass bestehender Code nicht bricht
    const account = {
        receiveAddress: addressStr,
        escrowAddress: escrowAddress,
        xpub: addressData.xpub,
        mnemonic: mnemonicPhrase,
        privateKeyHex,
        publicKey,
    };

    console.log(`✅ Kaspa Wallet (Raw Derivation) erfolgreich geladen! (Adresse: ${addressStr})`);

    // Das alte Wallet Construct ist fÃ¼r Raw Derivation ungenutzt.
    const wallet = null;

    return { wallet, account, address: addressStr };
}
export async function getBalance(account: any): Promise<number> {
    try {
        const mnemonicPhrase = account.mnemonic;
        if (!mnemonicPhrase) return 0;

        const rpcClient = await getRpcClient();

        // Re-derive generator to scan
        const mnemonic = new kaspa.Mnemonic(mnemonicPhrase);
        const seed = mnemonic.toSeed("");
        const xprv = new kaspa.XPrv(seed);
        const publicKeyGenerator = kaspa.PublicKeyGenerator.fromMasterXPrv(xprv as any, false, 0n);

        let totalSompi = 0n;
        const addressesToScan: string[] = [];

        // Add first 20 receive addresses
        for (let i = 0; i < 20; i++) {
            addressesToScan.push(publicKeyGenerator.receiveAddress(KASPA_NETWORK, i).toString());
        }
        // Add first 20 change addresses
        for (let i = 0; i < 20; i++) {
            addressesToScan.push(publicKeyGenerator.changeAddress(KASPA_NETWORK, i).toString());
        }

        // Fetch balances for all derived addresses
        for (const addr of addressesToScan) {
            try {
                const res = await rpcClient.getBalanceByAddress({ address: addr });
                if (res.balance) {
                    const mature = res.balance;
                    const bal = typeof mature === 'bigint' ? mature : BigInt(mature || 0);
                    totalSompi += bal;
                }
            } catch (e) {
                // Ignore API lookup errors for single empty addresses
            }
        }

        return Number(totalSompi);

    } catch (e) {
        console.warn("⚠️ Fehler beim Scannen der Balances", e);
        return 0;
    }
}

/**
 * Fetches balance for a single address via RPC
 */
export async function getBalanceByAddress(address: string): Promise<number> {
    try {
        const rpcClient = await getRpcClient();
        const res = await rpcClient.getBalanceByAddress({ address });
        if (res.balance) {
            const bal = typeof res.balance === 'bigint' ? res.balance : BigInt(res.balance || 0);
            return Number(bal);
        }
        return 0;
    } catch (e) {
        console.error(`[wallet] Error fetching balance for ${address}:`, e);
        throw e;
    }
}

// Deposit-TX an Escrow-Adresse senden
export async function sendDeposit(
    account: any,
    escrowAddress: string,
    amountSompi: number,
): Promise<string> {
    const mnemonicPhrase = account.mnemonic;
    if (!mnemonicPhrase) throw new Error("Mnemonic fehlt im Account-Objekt");

    console.log("🚀 Starte Deposit-Transaktion mit Multi-Address-Scan...");
    const rpc = await getRpcClient();

    // 1. Keys vorbereiten
    const mnemonic = new kaspa.Mnemonic(mnemonicPhrase);
    const seed = mnemonic.toSeed("");
    const xprv = new kaspa.XPrv(seed);
    const privateKeyGenerator = new kaspa.PrivateKeyGenerator(xprv, false, 0n);
    const publicKeyGenerator = kaspa.PublicKeyGenerator.fromMasterXPrv(xprv as any, false, 0n);

    const addressesToScan: string[] = [];
    const keyMap = new Map<string, kaspa.PrivateKey>();

    // Scan initialisieren (analog zu getBalance, scanne 20 receive + 20 change)
    for (let i = 0; i < 20; i++) {
        const addr = publicKeyGenerator.receiveAddress(KASPA_NETWORK, i).toString();
        addressesToScan.push(addr);
        keyMap.set(addr, privateKeyGenerator.receiveKey(i));

        const changeAddr = publicKeyGenerator.changeAddress(KASPA_NETWORK, i).toString();
        addressesToScan.push(changeAddr);
        keyMap.set(changeAddr, privateKeyGenerator.changeKey(i));
    }

    console.log(`🔍 Scanne ${addressesToScan.length} Adressen nach UTXOs...`);
    const { entries } = await rpc.getUtxosByAddresses(addressesToScan);

    if (!entries || entries.length === 0) {
        throw new Error("Kein Guthaben (UTXOs) auf dem Wallet gefunden. Bitte lade dein Wallet auf.");
    }

    // Sammle alle benötigten Private Keys für die gefundenen UTXOs
    const usedPrivateKeys: kaspa.PrivateKey[] = [];
    const usedKeyStrings = new Set<string>();

    for (const entry of entries) {
        const addr = entry.address?.toString();
        if (addr && keyMap.has(addr)) {
            const pk = keyMap.get(addr)!;
            const pkStr = pk.toString();
            if (!usedKeyStrings.has(pkStr)) {
                usedKeyStrings.add(pkStr);
                usedPrivateKeys.push(pk);
            }
        }
    }

    console.log(`📦 Gefundene UTXOs: ${entries.length}. Benötigte Keys: ${usedPrivateKeys.length}`);
    console.log(`🎯 Ziel (Escrow): ${escrowAddress}`);
    console.log(`💰 Betrag: ${amountSompi} sompi`);

    // 3. Transaktion erstellen
    const amount = BigInt(amountSompi);
    // Erster Receive-Address als Change-Adresse nutzen
    const changeAddress = publicKeyGenerator.receiveAddress(KASPA_NETWORK, 0).toString();

    let transactions;
    try {
        const result = await kaspa.createTransactions({
            entries,
            outputs: [{ address: escrowAddress, amount }],
            priorityFee: 0n,
            changeAddress,
            networkId: KASPA_NETWORK,
        });
        transactions = result.transactions;
        console.log(`✅ [sendDeposit] ${transactions.length} Transaktionen erstellt.`);
    } catch (e: any) {
        console.error("❌ [sendDeposit] Fehler bei createTransactions:", e);
        // WASM Fehler sind oft Strings oder haben keine message property
        const errorMsg = typeof e === 'string' ? e : (e?.message || JSON.stringify(e) || "Unbekannter WASM Fehler");
        throw new Error(`Transaktionserstellung fehlgeschlagen: ${errorMsg}`);
    }

    if (!transactions || transactions.length === 0) {
        throw new Error("Transaktionserstellung fehlgeschlagen: Keine Transaktionen generiert.");
    }

    // 4. Signieren und Senden
    let finalTxId = "";
    for (const [idx, pending] of transactions.entries()) {
        console.log(`✍️ [sendDeposit] Signiere Transaktion ${idx + 1}/${transactions.length}...`);
        try {
            await pending.sign(usedPrivateKeys);
        } catch (e: any) {
            console.error(`❌ [sendDeposit] Fehler beim Signieren von TX ${idx + 1}:`, e);
            throw new Error(`Signatur-Fehler: ${e.message || e}`);
        }

        console.log(`📤 [sendDeposit] Übermittle TX ${idx + 1} an RPC...`);
        try {
            finalTxId = await pending.submit(rpc);
            console.log(`✅ [sendDeposit] Transaktion ${idx + 1} gesendet! ID: ${finalTxId}`);
        } catch (e: any) {
            console.error(`❌ [sendDeposit] Fehler beim Senden von TX ${idx + 1}:`, e);
            throw new Error(`RPC-Submit-Fehler: ${e.message || e}`);
        }
    }

    return finalTxId;
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
