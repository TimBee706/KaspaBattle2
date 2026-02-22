import '@testing-library/jest-dom';
import { vi } from 'vitest';

// WASM SDK komplett mocken – im Test-Environment ist kein WASM verfügbar
vi.mock('kaspa-wasm', () => ({
    default: vi.fn().mockResolvedValue(undefined),  // init()
    RpcClient: vi.fn().mockImplementation(() => ({
        connect: vi.fn().mockResolvedValue(undefined),
        disconnect: vi.fn().mockResolvedValue(undefined),
        getInfo: vi.fn().mockResolvedValue({ isSynced: true, serverVersion: '0.15.0' }),
        getBalanceByAddress: vi.fn().mockResolvedValue({ balance: 5_000_000 }),  // 50 KAS
        getUtxosByAddresses: vi.fn().mockResolvedValue({ entries: [] }),
        submitTransaction: vi.fn().mockResolvedValue({ transactionId: 'mock-tx-hash-001' }),
        getFeeEstimate: vi.fn().mockResolvedValue({ feeRate: 1.0 }),
    })),
    Wallet: vi.fn().mockImplementation(() => ({
        createAccount: vi.fn().mockResolvedValue({
            externalAddress: vi.fn().mockReturnValue('kaspa:qz2ptjk67k2twpvhcqx2fpe3n24xklngrpsatdq4c4l5czlltest'),
            getBalance: vi.fn().mockResolvedValue(5_000_000),
            send: vi.fn().mockResolvedValue({ transactionId: 'mock-tx-hash-002' }),
        }),
        getAccount: vi.fn().mockResolvedValue({
            externalAddress: vi.fn().mockReturnValue('kaspa:qz2ptjk67k2twpvhcqx2fpe3n24xklngrpsatdq4c4l5czlltest'),
            getBalance: vi.fn().mockResolvedValue(5_000_000),
        }),
        addEventListener: vi.fn(),
    })),
    Mnemonic: vi.fn().mockImplementation(() => ({
        toString: () => 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about',
    })),
    NetworkId: { Mainnet: 'mainnet', Testnet: 'testnet' },
    Account: vi.fn(),
}));

// Axios mocken
vi.mock('axios', async () => {
    const actual = await vi.importActual('axios');
    return {
        ...actual,
        default: {
            create: vi.fn().mockReturnValue({
                get: vi.fn(),
                post: vi.fn(),
                interceptors: {
                    request: { use: vi.fn() },
                    response: { use: vi.fn() },
                },
            }),
        },
    };
});
