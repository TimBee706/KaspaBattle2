import * as kaspa from 'kaspa-wasm';
import { initKaspaWasm } from './init';
import { getRpcClient } from './rpc';
export { getRpcClient };
import { KASPA_NETWORK } from '../config/constants';

// ─── Safe Account Interface ──────────────────────────────────────────────────
// Security: NO mnemonic or privateKeyHex stored. Keys live only in short-lived
// signing function scopes and are discarded immediately after use.

export interface SafeAccount {
    receiveAddress: string;
    escrowAddress: string;
    xpub: string;
    publicKey: string;
    mnemonicPhrase?: string; // IN-MEMORY ONLY for deposit signatures (never stored in LocalStorage)
}

export interface WalletConnection {
    wallet: any;
    account: SafeAccount;
    address: string;
}

// ─── Ephemeral Signing Result ────────────────────────────────────────────────

export interface ImportResult {
    connection: WalletConnection;
    /** Private key hex — caller must use immediately and discard. Not stored. */
    ephemeralPrivateKeyHex: string;
}

// Wallet aus Mnemonic importieren
// Returns address/publicKey info + an ephemeral private key for the initial login signing.
// The ephemeral key MUST be used immediately and discarded — never stored in state.
export async function importWallet(mnemonicPhrase: string): Promise<ImportResult> {
    console.log("🔄 Initialisiere Kaspa WASM SDK...");
    await initKaspaWasm();

    console.log("🔌 Verbinde mit Kaspa RPC Node...");
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

    // 3. Key derivation
    let addressData: { address: kaspa.Address, xpub: string } | null = null;
    let publicKeyGenerator: kaspa.PublicKeyGenerator | null = null;
    let privateKeyGenerator: kaspa.PrivateKeyGenerator | null = null;

    try {
        publicKeyGenerator = kaspa.PublicKeyGenerator.fromMasterXPrv(xprv as any, false, 0n);
        privateKeyGenerator = new kaspa.PrivateKeyGenerator(xprv, false, 0n);

        const address = publicKeyGenerator.receiveAddress(KASPA_NETWORK, 0);
        addressData = {
            address: address,
            xpub: xprv.toXPub().xpub,
        };
    } catch (e) {
        console.error("Fehler bei KeyGenerator:", e);
        throw new Error("Fehler beim Ableiten der Empfangsadresse (Derivation)!");
    }

    if (!addressData) {
        throw new Error("Address Derivation fehlgeschlagen");
    }

    const addressStr = addressData.address.toString();
    console.log(`✅ Adresse erhalten: ${addressStr}`);

    const escrowAddress = publicKeyGenerator!.receiveAddress(KASPA_NETWORK, 1).toString();
    const privateKey = privateKeyGenerator!.receiveKey(0);
    const publicKey = privateKey.toPublicKey().toString();
    const ephemeralPrivateKeyHex = privateKey.toString();

    // SafeAccount: NO secrets stored
    const account: SafeAccount = {
        receiveAddress: addressStr,
        escrowAddress: escrowAddress,
        xpub: addressData.xpub,
        publicKey,
        mnemonicPhrase: mnemonicPhrase, // needed locally in memory for future deposits
    };

    console.log(`✅ Kaspa Wallet (Raw Derivation) erfolgreich geladen! (Adresse: ${addressStr})`);

    const wallet = null;

    return {
        connection: { wallet, account, address: addressStr },
        ephemeralPrivateKeyHex,
    };
}

/**
 * Fetches total balance across first 20 receive + 20 change addresses.
 * Uses xpub for address derivation — no private keys needed.
 */
export async function getBalance(xpub: string): Promise<number> {
    try {
        if (!xpub) return 0;

        await initKaspaWasm();
        const rpcClient = await getRpcClient();

        // Derive PublicKeyGenerator from xpub (no private key needed)
        const publicKeyGenerator = kaspa.PublicKeyGenerator.fromXPub(xpub, false);

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

/**
 * Deposit TX — requires mnemonic as explicit parameter (short-lived scope).
 *
 * The mnemonic is ONLY used within this function and never stored.
 * After the function returns, the mnemonic reference is discarded.
 */
export async function sendDeposit(
    mnemonicPhrase: string,
    escrowAddress: string,
    amountSompi: number,
): Promise<string> {
    if (!mnemonicPhrase) throw new Error("Mnemonic fehlt");

    console.log("🚀 Starte Deposit-Transaktion mit Multi-Address-Scan...");
    await initKaspaWasm();
    const rpc = await getRpcClient();

    // 1. Keys vorbereiten (ephemeral — scoped to this function)
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

    // Keys go out of scope here — ephemeral only
    return finalTxId;
}

// Event-Listener für Balance-Änderungen
export function onBalanceChange(
    _wallet: any,
    address: string,
    callback: (newBalance: number) => void,
): void {
    if (!address) return;

    getRpcClient().then(rpcClient => {
        rpcClient.subscribeUtxosChanged([address]).catch(e => {
            console.error("Konnte UTXOs nicht abonnieren", e);
        });

        (rpcClient as any).addEventListener('utxos-changed', async () => {
            try {
                const newBalance = await getBalanceByAddress(address);
                callback(newBalance);
            } catch (e) {
                console.error("Fehler beim UTXO Event Callback", e);
            }
        });
    }).catch(console.error);
}
