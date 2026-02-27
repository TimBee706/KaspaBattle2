import { useState } from 'react';
import { useWalletConnect } from '../hooks/useWalletConnect';
import { formatKas } from '../utils/format';

export function WalletPage() {
    const {
        isConnected, isConnecting, address, balanceSompi, error, mnemonic,
        connectWithMnemonic, disconnect,
    } = useWalletConnect();
    const [mnemonicInput, setMnemonicInput] = useState('');

    const handleImport = async (e?: React.FormEvent) => {
        if (e) e.preventDefault();
        if (mnemonicInput.trim().length > 0) {
            await connectWithMnemonic(mnemonicInput.trim());
            setMnemonicInput('');
        }
    };

    if (!isConnected || !address) {
        return (
            <div className="max-w-md mx-auto mt-20 p-8 card bg-kaspa-card border border-kaspa-border rounded-xl shadow-2xl animate-in fade-in slide-in-from-bottom-4">
                <div className="text-center mb-8">
                    <div className="text-5xl mb-4">🔑</div>
                    <h1 className="text-2xl font-black text-kaspa-primary uppercase tracking-tighter">Wallet importieren</h1>
                    <p className="text-gray-400 text-sm mt-2">
                        Bitte gib deine 12-Wort Seed-Phrase (Mnemonic) ein, um dein bestehendes Wallet wiederherzustellen.
                    </p>
                </div>

                <form onSubmit={handleImport} className="space-y-6">
                    <div>
                        <label className="block text-xs font-bold text-gray-400 uppercase tracking-widest mb-2">
                            Seed-Phrase (Mnemonic) oder Private Key
                        </label>
                        <textarea
                            value={mnemonicInput}
                            onChange={(e) => setMnemonicInput(e.target.value)}
                            disabled={isConnecting}
                            rows={3}
                            placeholder="word1 word2 word3..."
                            className="w-full bg-kaspa-dark border border-kaspa-border rounded-lg p-4 text-white resize-none focus:border-kaspa-primary outline-none transition-colors disabled:opacity-50"
                            autoFocus
                        />
                    </div>
                    {error && (
                        <div className="p-3 bg-red-900/20 border border-red-900/50 rounded-lg">
                            <p className="text-red-400 text-xs font-medium">{error}</p>
                        </div>
                    )}
                    <button
                        type="submit"
                        disabled={isConnecting || !mnemonicInput.trim()}
                        className="w-full py-4 bg-kaspa-primary hover:bg-kaspa-secondary disabled:bg-gray-700 text-kaspa-dark font-black text-lg rounded-xl transition-all shadow-lg active:scale-95 flex items-center justify-center gap-2"
                    >
                        {isConnecting ? (
                            <span className="animate-pulse">IMPORTIERE...</span>
                        ) : (
                            <>
                                <span>WALLET VERBINDEN</span>
                                <span>→</span>
                            </>
                        )}
                    </button>
                </form>
            </div>
        );
    }

    // Connected State: Wallet Overview
    return (
        <div className="max-w-4xl mx-auto space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500">
            <div className="flex justify-between items-center px-4">
                <h1 className="text-3xl font-black tracking-tighter">WALLET <span className="text-kaspa-primary">ÜBERSICHT</span></h1>
                <button
                    onClick={disconnect}
                    className="px-4 py-2 bg-red-900/20 hover:bg-red-900/40 text-red-500 text-sm font-bold rounded-lg border border-red-900/30 transition-colors"
                >
                    Wallet trennen
                </button>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                <div className="card p-8 bg-gradient-to-br from-kaspa-card to-kaspa-dark border border-kaspa-primary/30 relative overflow-hidden">
                    <div className="absolute top-0 right-0 w-32 h-32 bg-kaspa-primary/10 rounded-full blur-3xl" />
                    <h3 className="text-xs font-black text-kaspa-primary uppercase tracking-widest mb-2">Guthaben</h3>
                    <div className="text-5xl font-black mb-4">
                        {formatKas(balanceSompi)} <span className="text-xl text-gray-400">KAS</span>
                    </div>
                </div>

                <div className="card p-8 flex flex-col justify-center">
                    <h3 className="text-xs font-black text-gray-500 uppercase tracking-widest mb-2">Kaspa Adresse</h3>
                    <div className="bg-kaspa-dark p-4 rounded-lg border border-kaspa-border break-all font-mono text-sm text-kaspa-primary select-all mb-4">
                        {address}
                    </div>
                    <button
                        onClick={() => navigator.clipboard.writeText(address)}
                        className="text-xs font-bold text-gray-400 hover:text-white flex items-center gap-2 transition-colors self-start"
                    >
                        <span>📋</span> Adresse kopieren
                    </button>
                </div>
            </div>

            <div className="card p-8">
                <h3 className="text-sm font-black text-gray-500 uppercase tracking-widest mb-6 border-l-4 border-kaspa-primary pl-4">Transaktionsverlauf</h3>
                <div className="flex justify-center items-center h-32 text-gray-500 text-sm italic bg-kaspa-dark/50 rounded-lg border border-kaspa-border/50">
                    Transaktionshistorie wird in Kürze verfügbar sein.
                </div>
            </div>

            {mnemonic && (
                <div className="card p-8 border border-red-900/20 bg-red-900/5">
                    <div className="flex items-center gap-3 mb-6">
                        <span className="text-2xl">⚠️</span>
                        <h3 className="text-sm font-black text-red-500 uppercase tracking-widest">Secret Recovery Phrase (Backup)</h3>
                    </div>
                    <p className="text-gray-400 text-sm mb-6 leading-relaxed">
                        Diese Phrase ist der einzige Weg, dein Guthaben wiederherzustellen. Teile sie NIEMALS mit anderen.
                        Mitarbeiter von KaspaBattle werden dich niemals danach fragen.
                    </p>
                    <div className="bg-kaspa-dark p-6 rounded-xl border border-kaspa-border relative group">
                        <div className="flex flex-wrap gap-2 blur-md group-hover:blur-none transition-all duration-300">
                            {mnemonic.split(' ').map((word: string, i: number) => (
                                <div key={i} className="bg-kaspa-card/50 px-3 py-1.5 rounded-lg border border-kaspa-border flex gap-2">
                                    <span className="text-gray-500 text-xs">{i + 1}</span>
                                    <span className="font-mono text-kaspa-primary font-bold">{word}</span>
                                </div>
                            ))}
                        </div>
                        <div className="absolute inset-0 flex items-center justify-center group-hover:hidden bg-kaspa-dark/80 backdrop-blur-sm rounded-xl transition-opacity">
                            <span className="text-xs font-black tracking-widest text-gray-400">HOVER ZUM ANZEIGEN</span>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
