import { useEscrowDeposit } from '../../hooks/useEscrowDeposit';
import { useWalletStore } from '../../stores/useWalletStore';
import { formatKas, explorerTxUrl } from '../../utils/format';
import { useTranslation } from 'react-i18next';
import { hasValidBalance } from '../../utils/walletBalance';

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
    const { t } = useTranslation();

    if (!isOpen) return null;

    console.log('[DepositModal] amountSompi:', amountSompi, 'balanceSompi:', balanceSompi);
    const hasBalance = hasValidBalance(balanceSompi);
    const effectiveBalanceSompi = hasBalance ? balanceSompi : 0;
    const hasEnoughBalance = effectiveBalanceSompi >= (amountSompi || 0);

    return (
        <div className="fixed inset-0 z-[100] flex items-center justify-center p-4">
            <div className="absolute inset-0 bg-kaspa-dark/90 backdrop-blur-sm" onClick={!isDepositing ? onClose : undefined} />

            <div className="card w-full max-w-md relative z-10 animate-in zoom-in-95 duration-200">
                <h3 className="text-xl font-bold mb-4 uppercase tracking-tight">{t('deposit.confirm_title')}</h3>

                {!depositTxHash ? (
                    <div className="space-y-6">
                        <p className="text-gray-400 text-sm">
                            {t('deposit.info')}
                        </p>

                        <div className="bg-kaspa-dark rounded-xl p-4 space-y-3 border border-kaspa-border">
                            <div className="flex justify-between text-xs">
                                <span className="text-gray-500 uppercase font-bold">{t('deposit.amount')}</span>
                                <span className="text-white font-black">{formatKas(amountSompi)} KAS</span>
                            </div>
                            <div className="flex justify-between text-xs">
                                <span className="text-gray-500 uppercase font-bold">{t('deposit.your_balance')}</span>
                                <span className={`font-black ${hasEnoughBalance ? 'text-kaspa-primary' : 'text-red-500'}`}>
                                    {hasBalance ? formatKas(balanceSompi) : '--'} KAS
                                </span>
                            </div>
                        </div>

                        {!hasEnoughBalance && (
                            <div className="p-3 bg-red-900/20 border border-red-900/50 rounded-lg flex gap-3">
                                <svg xmlns="http://www.w3.org/2000/svg" className="w-5 h-5 text-red-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.75}><path strokeLinecap="round" strokeLinejoin="round" d="M12 9v4m0 4h.01M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z" /></svg>
                                <div>
                                    <p className="text-red-400 text-xs font-bold">{t('deposit.insufficient_funds')}</p>
                                    <p className="text-red-300/70 text-[10px]">{t('deposit.insufficient_funds_info')}</p>
                                </div>
                            </div>
                        )}

                        <div className="flex gap-3">
                            <button
                                onClick={onClose}
                                disabled={isDepositing}
                                className="flex-1 py-3 bg-kaspa-border hover:bg-gray-700 rounded-xl text-sm font-bold transition-colors"
                            >
                                {t('deposit.cancel')}
                            </button>
                            <button
                                onClick={() => executeDeposit(playerRole)}
                                disabled={isDepositing || !hasEnoughBalance}
                                className="flex-[2] btn-primary py-3 flex items-center justify-center gap-2"
                            >
                                {isDepositing ? (
                                    <>
                                        <div className="w-4 h-4 border-2 border-kaspa-dark border-t-transparent rounded-full animate-spin" />
                                        {t('deposit.signing')}
                                    </>
                                ) : t('deposit.pay_now')}
                            </button>
                        </div>
                    </div>
                ) : (
                    <div className="text-center py-6 animate-in fade-in slide-in-from-bottom-2">
                        <div className="w-16 h-16 bg-kaspa-primary/20 text-kaspa-primary rounded-full flex items-center justify-center mx-auto mb-6">
                            <svg xmlns="http://www.w3.org/2000/svg" className="w-8 h-8" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}><path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" /></svg>
                        </div>
                        <h4 className="text-lg font-bold mb-2 uppercase">{t('deposit.success_title')}</h4>
                        <p className="text-gray-400 text-xs mb-8">
                            {t('deposit.success_info')}
                        </p>
                        <a
                            href={explorerTxUrl(depositTxHash)}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="block w-full py-3 border border-kaspa-primary text-kaspa-primary text-xs font-bold rounded-xl mb-4 hover:bg-kaspa-primary/10 transition-colors"
                        >
                            {t('deposit.view_explorer')}
                        </a>
                        <button
                            onClick={onClose}
                            className="w-full py-2 text-gray-500 text-xs font-bold hover:text-white transition-colors"
                        >
                            {t('deposit.close')}
                        </button>
                    </div>
                )}
            </div>
        </div>
    );
}
