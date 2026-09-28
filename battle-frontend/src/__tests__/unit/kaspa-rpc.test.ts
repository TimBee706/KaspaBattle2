import { describe, it, expect, vi, beforeEach } from 'vitest';

describe('kaspa/rpc getRpcClient', () => {
    beforeEach(() => {
        vi.resetModules();
        vi.clearAllMocks();
    });

    it('uses the Resolver when no direct node URL is configured', async () => {
        vi.doMock('../../config/constants', () => ({
            KASPA_NODE_URL: '',
            KASPA_NETWORK: 'testnet-10',
        }));
        const connectMock = vi.fn().mockResolvedValue(undefined);
        const resolverCtor = vi.fn().mockImplementation(() => ({}));
        vi.doMock('kaspa-wasm', () => ({
            default: vi.fn().mockResolvedValue(undefined),
            Encoding: { Borsh: 0 },
            ConnectStrategy: { Retry: 0, Fallback: 1 },
            Resolver: resolverCtor,
            RpcClient: vi.fn().mockImplementation((config: Record<string, unknown>) => ({
                config,
                connect: connectMock,
                disconnect: vi.fn(),
            })),
        }));

        const { getRpcClient } = await import('../../kaspa/rpc');
        const client = await getRpcClient() as unknown as { config: Record<string, unknown> };

        expect(resolverCtor).toHaveBeenCalledTimes(1);
        expect(client.config.resolver).toBeDefined();
        expect(client.config.url).toBeUndefined();
        expect(connectMock).toHaveBeenCalledWith(expect.objectContaining({ strategy: 1 }));
    });

    it('uses the direct URL when VITE_KASPA_NODE_URL is set, without touching the Resolver', async () => {
        vi.doMock('../../config/constants', () => ({
            KASPA_NODE_URL: 'wss://example-node:17110',
            KASPA_NETWORK: 'testnet-10',
        }));
        const connectMock = vi.fn().mockResolvedValue(undefined);
        const resolverCtor = vi.fn();
        vi.doMock('kaspa-wasm', () => ({
            default: vi.fn().mockResolvedValue(undefined),
            Encoding: { Borsh: 0 },
            ConnectStrategy: { Retry: 0, Fallback: 1 },
            Resolver: resolverCtor,
            RpcClient: vi.fn().mockImplementation((config: Record<string, unknown>) => ({
                config,
                connect: connectMock,
                disconnect: vi.fn(),
            })),
        }));

        const { getRpcClient } = await import('../../kaspa/rpc');
        const client = await getRpcClient() as unknown as { config: Record<string, unknown> };

        expect(resolverCtor).not.toHaveBeenCalled();
        expect(client.config.url).toBe('wss://example-node:17110');
    });

    it('throws a KaspaRpcError with technical detail when both connection attempts fail', async () => {
        vi.doMock('../../config/constants', () => ({
            KASPA_NODE_URL: '',
            KASPA_NETWORK: 'testnet-10',
        }));
        vi.doMock('kaspa-wasm', () => ({
            default: vi.fn().mockResolvedValue(undefined),
            Encoding: { Borsh: 0 },
            ConnectStrategy: { Retry: 0, Fallback: 1 },
            Resolver: vi.fn().mockImplementation(() => ({})),
            RpcClient: vi.fn().mockImplementation(() => ({
                connect: vi.fn().mockRejectedValue(new Error('network unreachable')),
                disconnect: vi.fn(),
            })),
        }));

        const { getRpcClient } = await import('../../kaspa/rpc');
        const { KaspaRpcError } = await import('../../kaspa/errors');

        let caught: unknown;
        try {
            await getRpcClient();
        } catch (e) {
            caught = e;
        }

        expect(caught).toBeInstanceOf(KaspaRpcError);
        expect((caught as InstanceType<typeof KaspaRpcError>).code).toBe('KASPA_RPC_UNAVAILABLE');
        expect((caught as InstanceType<typeof KaspaRpcError>).technicalDetail).toContain('network unreachable');
    });
});

describe('kaspa/wallet importWallet — RPC error propagation', () => {
    beforeEach(() => {
        vi.resetModules();
        vi.clearAllMocks();
    });

    it('rethrows a KaspaRpcError instead of swallowing the real connection failure', async () => {
        vi.doMock('kaspa-wasm', () => ({
            default: vi.fn().mockResolvedValue(undefined),
        }));
        vi.doMock('../../kaspa/rpc', () => ({
            getRpcClient: vi.fn().mockRejectedValue(new Error('resolver timed out')),
        }));

        const { importWallet } = await import('../../kaspa/wallet');
        const { KaspaRpcError } = await import('../../kaspa/errors');

        let caught: unknown;
        try {
            await importWallet('test phrase');
        } catch (e) {
            caught = e;
        }

        expect(caught).toBeInstanceOf(KaspaRpcError);
        expect((caught as InstanceType<typeof KaspaRpcError>).technicalDetail).toContain('resolver timed out');
    });
});
