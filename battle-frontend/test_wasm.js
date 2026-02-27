import * as kaspa from 'kaspa-wasm';
import initWasm from 'kaspa-wasm';

async function run() {
    try {
        await initWasm();
        const mnemonic = kaspa.Mnemonic.random();
        console.log("Mnemonic created:", mnemonic.phrase);

        try {
            const wallet = new kaspa.Wallet(null);
            console.log("Wallet instantiated.");

            /// from nodejs documentation for kaspa-wasm Wallet:
            // What is the wallet argument? 
            // `new kaspa.Wallet({ resident: true, networkId: new kaspa.NetworkId('testnet-10') })` 
        } catch (e) {
            console.log("Failed w1:", e);
        }

        try {
            const networkId = new kaspa.NetworkId('testnet-10');
            const wallet = new kaspa.Wallet({ resident: true, networkId });
            console.log("Second Wallet instantiated.");

            // check createPrvKeyData
            const prvKeyDataInfo = await wallet.createPrvKeyData({
                mnemonic: mnemonic.phrase,
                password: ""
            });
            console.log("PrvKeyDataInfo:", prvKeyDataInfo);

            // createAccount
            const account = await wallet.createAccount({
                prvKeyDataId: prvKeyDataInfo.id,
                name: 'KaspaBattle',
                accountKind: kaspa.AccountKind.Bip32 // 1
            });
            console.log("Account created:", account.receiveAddress);
            console.log("Done");
        } catch (e) {
            console.log("Failed API flow:", e);
        }
    } catch (e) {
        console.error("Error:", e);
    }
}
run();
