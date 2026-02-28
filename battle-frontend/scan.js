import * as kaspa from 'kaspa-wasm';

async function scan() {
    console.log("Init WASM...");
    await kaspa.default();

    console.log("Connect RPC...");
    const rpcClient = new kaspa.RpcClient({
        url: 'wss://photon-10.kaspa.red/kaspa/testnet-10/wrpc/borsh',
        encoding: kaspa.Encoding.Borsh,
        networkId: 'testnet-10'
    });
    await rpcClient.connect();

    const mnemonicPhrase = 'poem spring clock goat nothing comfort scan purpose review offer fresh distance';
    const mnemonic = new kaspa.Mnemonic(mnemonicPhrase);
    const seed = mnemonic.toSeed("");
    const xprv = new kaspa.XPrv(seed);
    const publicKeyGenerator = kaspa.PublicKeyGenerator.fromMasterXPrv(xprv, false, 0n);

    console.log("Scanning Receive Addresses...");
    let total = 0n;
    for (let i = 0; i < 20; i++) {
        const addr = publicKeyGenerator.receiveAddress('testnet-10', i);
        const res = await rpcClient.getBalanceByAddress({ address: addr.toString() });
        const bal = typeof res.balance === 'bigint' ? res.balance : BigInt(res.balance || 0);
        if (bal > 0n) {
            console.log(`Found ${bal} on receive[${i}]: ${addr.toString()}`);
            total += bal;
        }
    }

    console.log("Scanning Change Addresses...");
    for (let i = 0; i < 20; i++) {
        const addr = publicKeyGenerator.changeAddress('testnet-10', i);
        const res = await rpcClient.getBalanceByAddress({ address: addr.toString() });
        const bal = typeof res.balance === 'bigint' ? res.balance : BigInt(res.balance || 0);
        if (bal > 0n) {
            console.log(`Found ${bal} on change[${i}]: ${addr.toString()}`);
            total += bal;
        }
    }

    console.log(`Total Sompi found: ${total}`);
    await rpcClient.disconnect();
}

scan().catch(console.error);
