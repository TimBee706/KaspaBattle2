import { useEscrowDeposit } from '../../hooks/useEscrowDeposit';
import { useWalletStore } from '../../stores/useWalletStore';
import { formatKas, explorerTxUrl } from '../../utils/format';

interface DepositConfirmModalProps {
    isOpen: boolean;
    onClose: () => void;
    matchId: string;
    amountSompi: number;
    playerRole: 'A' | 'B';
}

export function DepositConfirmModal({ isOpen, onClose, amountSompi, playerRole }: DepositConfirmModalProps) {
    const { executeDeposit, isDepositing, depositTxHash } = useEscrowDeposit();
    const { balanceSompi } = useWalletStore();

    if (!isOpen) return null;

    console.log('[DepositModal] amountSompi:', amountSompi, 'balanceSompi:', balanceSompi);
    const hasEnoughBalance = (balanceSompi || 0) >= (amountSompi || 0);

    return (
        <div className="fixed inset-0 z-[100] flex items-center justify-center p-4">
            <div className="absolute inset-0 bg-kaspa-dark/90 backdrop-blur-sm" onClick={!isDepositing ? onClose : undefined} />

            <div className="card w-full max-w-md relative z-10 animate-in zoom-in-95 duration-200">
                <h3 className="text-xl font-bold mb-4 uppercase tracking-tight">Einzahlung bestätigen</h3>

                {!depositTxHash ? (
                    <div className="space-y-6">
                        <p className="text-gray-400 text-sm">
                            Du zahlst deinen Einsatz in die Escrow-Adresse des Matches ein. Die Funds werden dort gesperrt, bis der Sieger feststeht.
                        </p>

                        <div className="bg-kaspa-dark rounded-xl p-4 space-y-3 border border-kaspa-border">
                            <div className="flex justify-between text-xs">
                                <span className="text-gray-500 uppercase font-bold">Betrag</span>
                                <span className="text-white font-black">{formatKas(amountSompi)} KAS</span>
                            </div>
                            <div className="flex justify-between text-xs">
                                <span className="text-gray-500 uppercase font-bold">Deine Balance</span>
                                <span className={`font-black ${hasEnoughBalance ? 'text-kaspa-primary' : 'text-red-500'}`}>
                                    {formatKas(balanceSompi)} KAS
                                </span>
                            </div>
                        </div>

                        {!hasEnoughBalance && (
                            <div className="p-3 bg-red-900/20 border border-red-900/50 rounded-lg flex gap-3">
                                <span className="text-xl">⚠️</span>
                                <div>
                                    <p className="text-red-400 text-xs font-bold">Unzureichendes Guthaben</p>
                                    <p className="text-red-300/70 text-[10px]">Bitte lade dein Kaspa-Wallet auf, um fortzufahren.</p>
                                </div>
                            </div>
                        )}

                        <div className="flex gap-3">
                            <button
                                onClick={onClose}
                                disabled={isDepositing}
                                className="flex-1 py-3 bg-kaspa-border hover:bg-gray-700 rounded-xl text-sm font-bold transition-colors"
                            >
                                ABBRECHEN
                            </button>
                            <button
                                onClick={() => executeDeposit(playerRole)}
                                disabled={isDepositing || !hasEnoughBalance}
                                className="flex-[2] btn-primary py-3 flex items-center justify-center gap-2"
                            >
                                {isDepositing ? (
                                    <>
                                        <div className="w-4 h-4 border-2 border-kaspa-dark border-t-transparent rounded-full animate-spin" />
                                        SIGNIEREN...
                                    </>
                                ) : 'JETZT EINZAHLEN'}
                            </button>
                        </div>
                    </div>
                ) : (
                    <div className="text-center py-6 animate-in fade-in slide-in-from-bottom-2">
                        <div className="w-16 h-16 bg-kaspa-primary/20 text-kaspa-primary rounded-full flex items-center justify-center text-3xl mx-auto mb-6">✓</div>
                        <h4 className="text-lg font-bold mb-2 uppercase">Einzahlung erfolgreich!</h4>
                        <p className="text-gray-400 text-xs mb-8">
                            Deine Transaktion wurde an das Kaspa-Netzwerk gesendet. Das Match-Status wird in Kürze aktualisiert.
                        </p>
                        <a
                            href={explorerTxUrl(depositTxHash)}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="block w-full py-3 border border-kaspa-primary text-kaspa-primary text-xs font-bold rounded-xl mb-4 hover:bg-kaspa-primary/10 transition-colors"
                        >
                            TRANSATION IM EXPLORER ANSEHEN
                        </a>
                        <button
                            onClick={onClose}
                            className="w-full py-2 text-gray-500 text-xs font-bold hover:text-white transition-colors"
                        >
                            SCHLIESSEN
                        </button>
                    </div>
                )}
            </div>
        </div>
    );
}
