import { useState } from 'react';
import { useWallet } from '../hooks/useWallet';
import { useTranslation } from 'react-i18next';
import { Icon } from '../components/Icon';
import { TestnetComingSoon } from '../components/common/TestnetComingSoon';

export function WalletPage() {
    const {
        isConnected,
        isConnecting,
        address,
        balanceSompi,
        isFetchingBalance,
        balanceError,
        error,
        connectWithMnemonic,
        disconnect,
        fetchBalance,
    } = useWallet();

    const [mnemonicInput, setMnemonicInput] = useState('');
    const { t } = useTranslation();

    const handleImport = async (e?: React.FormEvent) => {
        if (e) e.preventDefault();
        if (mnemonicInput.trim().length > 0) {
            await connectWithMnemonic(mnemonicInput.trim());
            setMnemonicInput('');
        }
    };

    if (!isConnected || !address) {
        return (
            <div>
                {/* Header — mirrors LobbyPage exactly */}
                <div className="flex flex-col md:flex-row md:justify-between items-start md:items-center gap-6 mb-12">
                    <div>
                        <h1 className="text-4xl font-black text-white uppercase tracking-tighter mb-2">{t('wallet.import_title')}</h1>
                        <p className="text-slate-500 text-sm font-bold uppercase tracking-widest">{t('wallet.import_subtitle')}</p>
                    </div>
                </div>

                <TestnetComingSoon className="mb-6" />

                {/* Section box — same style as LobbyTable */}
                <div className="max-w-lg mx-auto mb-8 p-6 glass-panel rounded-2xl border border-kaspa-primary/20 shadow-glow-primary">

                    <div className="max-w-md mx-auto p-8">
                        <div className="text-center mb-8">
                            <div className="w-14 h-14 rounded-2xl bg-kaspa-primary/10 border border-kaspa-primary/20 flex items-center justify-center mx-auto mb-4">
                                <Icon name="wallet" className="w-7 h-7 text-kaspa-primary" />
                            </div>
                        </div>

                        <form onSubmit={handleImport} className="space-y-6">
                            <div>
                                <label className="block text-xs font-bold text-gray-400 uppercase tracking-widest mb-2">
                                    {t('wallet.seed_label')}
                                </label>
                                <textarea
                                    value={mnemonicInput}
                                    onChange={(e) => setMnemonicInput(e.target.value)}
                                    disabled={isConnecting}
                                    rows={3}
                                    placeholder={t('wallet.placeholder')}
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
                                    <span className="animate-pulse">{t('wallet.importing')}</span>
                                ) : (
                                    <>
                                        <span>{t('wallet.connect')}</span>
                                        <span>→</span>
                                    </>
                                )}
                            </button>
                        </form>
                    </div>
                </div>
            </div>
        );
    }

    const balance = balanceSompi / 100_000_000;

    return (
        <div>
            {/* Header — mirrors LobbyPage exactly */}
            <div className="flex flex-col md:flex-row md:justify-between items-start md:items-center gap-6 mb-12">
                <div>
                    <h1 className="text-4xl font-black text-white uppercase tracking-tighter mb-2">{t('wallet.overview')}</h1>
                    <p className="text-slate-500 text-sm font-bold uppercase tracking-widest">{t('wallet.address')}</p>
                </div>
                <div className="flex gap-4">
                    <button
                        onClick={() => fetchBalance(address)}
                        disabled={isFetchingBalance}
                        className="glass-button px-4 py-2 text-sm font-bold flex items-center gap-2 disabled:opacity-50"
                    >
                        <Icon name="refresh" className="w-4 h-4" /> {t('wallet.refresh')}
                    </button>
                    <button
                        onClick={disconnect}
                        className="px-4 py-2 bg-red-900/20 hover:bg-red-900/40 text-red-500 text-sm font-bold rounded-lg border border-red-900/30 transition-colors"
                    >
                        {t('wallet.disconnect')}
                    </button>
                </div>
            </div>

            {/* Section box — same style as LobbyTable */}
            <div className="mb-8 p-6 glass-panel rounded-2xl border border-kaspa-primary/20 shadow-glow-primary">
                <h2 className="text-xl font-black mb-6 text-emerald-400 uppercase tracking-tighter pl-2">{t('wallet.overview')}</h2>

                <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                    <div className="glass-panel p-8 bg-gradient-to-br from-kaspa-surface to-kaspa-dark border border-white/5 rounded-2xl relative overflow-hidden">
                        <div className="absolute top-0 right-0 w-32 h-32 bg-kaspa-primary/10 rounded-full blur-3xl" />
                        <h3 className="text-xs font-black text-kaspa-primary uppercase tracking-widest mb-2">{t('wallet.balance')}</h3>
                        <div className="text-5xl font-black mb-4 flex items-baseline gap-2">
                            {isFetchingBalance && balanceSompi === -1 ? (
                                <span className="animate-pulse text-gray-500">...</span>
                            ) : balanceError ? (
                                <span className="text-red-500 text-3xl">—</span>
                            ) : (
                                <>
                                    <span>{balance.toLocaleString('de-DE', { minimumFractionDigits: 2 })}</span>
                                    <span className="text-xl text-gray-400 ml-2">KAS</span>
                                </>
                            )}
                        </div>
                    </div>

                    <div className="glass-panel p-8 flex flex-col justify-center border border-white/5 rounded-2xl">
                        <h3 className="text-xs font-black text-gray-500 uppercase tracking-widest mb-2">{t('wallet.address')}</h3>
                        <div className="bg-kaspa-dark p-4 rounded-lg border border-kaspa-border break-all font-mono text-sm text-kaspa-primary select-all mb-4">
                            {address}
                        </div>
                        <button
                            onClick={() => navigator.clipboard.writeText(address)}
                            className="text-xs font-bold text-gray-400 hover:text-white flex items-center gap-2 transition-colors self-start"
                        >
                            <Icon name="copy" className="w-3.5 h-3.5" /> {t('wallet.copy_address')}
                        </button>
                    </div>
                </div>

                <div className="glass-panel p-8 mt-4 border border-white/5 rounded-2xl">
                    <h3 className="text-sm font-black text-gray-500 uppercase tracking-widest mb-6 border-l-4 border-kaspa-primary pl-4">{t('wallet.tx_history')}</h3>
                    <div className="flex justify-center items-center h-32 text-gray-500 text-sm italic bg-kaspa-dark/50 rounded-lg border border-kaspa-border/50">
                        {t('wallet.tx_history_empty')}
                    </div>
                </div>
            </div>
        </div>
    );
}
