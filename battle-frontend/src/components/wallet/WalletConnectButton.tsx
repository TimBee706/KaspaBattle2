import { useState } from 'react';
import { useWalletConnect } from '../../hooks/useWalletConnect';
import { formatKas, shortenAddress } from '../../utils/format';

export function WalletConnectButton() {
    const {
        isConnected, isConnecting, address, balanceSompi, error,
        connectNewWallet, connectWithMnemonic, disconnect,
    } = useWalletConnect();
    const [showImport, setShowImport] = useState(false);
    const [mnemonicInput, setMnemonicInput] = useState('');
    const [savedMnemonic, setSavedMnemonic] = useState<string | null>(null);

    if (isConnected && address) {
        return (
            <div className="flex items-center gap-3 bg-kaspa-dark/50 border border-kaspa-border px-3 py-1.5 rounded-lg shadow-inner">
                <div className="text-sm flex flex-col items-end">
                    <span className="text-kaspa-primary font-bold text-sm">
                        {formatKas(balanceSompi)} KAS
                    </span>
                    <span className="text-gray-500 font-mono text-[10px]">
                        {shortenAddress(address)}
                    </span>
                </div>
                <button
                    onClick={disconnect}
                    className="ml-2 w-8 h-8 flex items-center justify-center bg-red-900/30 hover:bg-red-900/50 text-red-500 border border-red-900/50 rounded-md transition-colors"
                    title="Wallet trennen"
                >
                    ✕
                </button>
            </div>
        );
    }

    const handleCreateNew = async () => {
        const mnemonic = await connectNewWallet();
        if (mnemonic) setSavedMnemonic(mnemonic);
    };

    const handleImport = async () => {
        if (mnemonicInput.trim().split(/\s+/).length === 12) {
            await connectWithMnemonic(mnemonicInput.trim());
            setShowImport(false);
            setMnemonicInput('');
        }
    };

    return (
        <div className="relative">
            {savedMnemonic && (
                <div className="absolute top-12 right-0 z-[60] bg-kaspa-card border-2 border-yellow-600 rounded-xl p-5 w-80 shadow-2xl animate-in fade-in slide-in-from-top-4 duration-300">
                    <div className="flex items-center gap-2 text-yellow-500 font-bold mb-3">
                        <span>⚠️</span>
                        <span className="uppercase tracking-wider text-xs">Mnemonic sichern!</span>
                    </div>
                    <p className="text-gray-400 text-[10px] leading-relaxed mb-4">
                        Schreibe diese 12 Wörter auf. Ohne sie kannst du niemals wieder auf dein Guthaben zugreifen!
                    </p>
                    <div className="bg-kaspa-dark p-3 rounded-lg border border-kaspa-border text-center font-mono text-sm text-yellow-100 break-words select-all mb-4">
                        {savedMnemonic}
                    </div>
                    <button
                        onClick={() => {
                            navigator.clipboard.writeText(savedMnemonic);
                            setSavedMnemonic(null);
                        }}
                        className="w-full py-2 bg-yellow-600 hover:bg-yellow-700 text-kaspa-dark font-bold rounded-lg text-xs"
                    >
                        Kopieren & Bestätigen
                    </button>
                </div>
            )}

            {error && <p className="text-red-400 text-[10px] absolute -bottom-4 right-0 font-medium">{error}</p>}

            {showImport ? (
                <div className="flex gap-2">
                    <input
                        type="password"
                        value={mnemonicInput}
                        onChange={(e) => setMnemonicInput(e.target.value)}
                        placeholder="12-Wort Mnemonic"
                        className="px-3 py-1.5 text-xs bg-kaspa-dark border border-kaspa-border rounded-lg text-white w-48 focus:border-kaspa-primary outline-none transition-colors"
                        autoFocus
                    />
                    <button
                        onClick={handleImport}
                        className="px-3 py-1.5 text-xs bg-kaspa-primary text-kaspa-dark font-bold rounded-lg hover:bg-kaspa-secondary transition-colors"
                    >
                        Import
                    </button>
                    <button onClick={() => setShowImport(false)} className="px-2 text-gray-500 hover:text-white">✕</button>
                </div>
            ) : (
                <div className="flex gap-2">
                    <button
                        onClick={handleCreateNew}
                        disabled={isConnecting}
                        className="px-4 py-2 bg-kaspa-primary hover:bg-kaspa-secondary disabled:bg-gray-700 text-kaspa-dark font-bold rounded-lg text-sm transition-all shadow-lg active:scale-95"
                    >
                        {isConnecting ? '...' : 'Neues Wallet'}
                    </button>
                    <button
                        onClick={() => setShowImport(true)}
                        className="px-4 py-2 bg-kaspa-card border border-kaspa-border hover:bg-kaspa-border text-white rounded-lg text-sm transition-all"
                    >
                        Import
                    </button>
                </div>
            )}
        </div>
    );
}
