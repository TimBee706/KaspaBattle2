import React, { useState } from 'react';
import { useAuthStore } from '../stores/useAuthStore';
import { useLobby } from '../hooks/useLobby';
import { useWallet } from '../hooks/useWallet';
import { useTranslation } from 'react-i18next';
import { FEATURE_FLAGS } from '../config/featureFlags';
import type { MatchMode } from '../api/types';
import { Icon } from './Icon';

export const ChallengeModal: React.FC = () => {
    const [open, setOpen] = useState(false);
    const [stake, setStake] = useState<number | string>(50);
    const [mode, setMode] = useState<'BO1' | 'BO3'>('BO1');
    const { user } = useAuthStore();
    const { isConnected, balanceSompi } = useWallet();
    const { createChallenge, isCreating, minWager, canCreateChallenge } = useLobby();
    const { t } = useTranslation();

    const faceitId = user?.faceit_id || null;
    const balanceKas = balanceSompi / 100_000_000;

    // Derived flags for cleaner JSX
    const isFaceIdMissing = !FEATURE_FLAGS.TEST_MODE && !faceitId;
    const isBalanceLow = balanceKas < minWager;
    const canSubmit = canCreateChallenge && (!FEATURE_FLAGS.TEST_MODE ? !!faceitId : true);

    const submit = async (e: React.FormEvent) => {
        e.preventDefault();
        try {
            await createChallenge({
                stakeKas: Number(stake),
                mode: mode
            });
            setOpen(false);
        } catch (e) {
            console.error(e);
        }
    };

    if (!open) {
        const disabledReason = !isConnected
            ? t('challenge.connect_wallet_hint')
            : isFaceIdMissing
                ? t('challenge.connect_faceit_hint')
                : isBalanceLow
                    ? t('escrow.low_balance_hint', { amount: minWager })
                    : null;

        return (
            <div className="flex flex-col items-end gap-2">
                <button
                    onClick={() => setOpen(true)}
                    disabled={!canSubmit}
                    title={disabledReason || t('challenge.create')}
                    className="bg-kaspa-primary hover:bg-kaspa-secondary text-kaspa-dark px-8 py-3 rounded-xl font-black uppercase tracking-tighter disabled:opacity-30 disabled:grayscale transition-all shadow-xl shadow-kaspa-primary/10 active:scale-95"
                >
                    {t('challenge.create')}
                </button>
                {disabledReason && (
                    <span className="text-[10px] font-black text-red-500/60 uppercase tracking-widest flex items-center gap-1">
                        <Icon name="alert-triangle" className="w-3 h-3 shrink-0" /> {disabledReason}
                    </span>
                )}
            </div>
        );
    }

    return (
        <div className="fixed inset-0 bg-black/80 backdrop-blur-md flex items-center justify-center z-[110] p-4 text-white animate-in fade-in duration-200">
            <form onSubmit={submit} className="bg-slate-900 p-8 rounded-2xl w-full max-w-md border border-slate-700 shadow-2xl animate-in zoom-in-95 duration-200">
                <h2 className="text-2xl font-black mb-8 uppercase tracking-tighter border-b border-slate-800 pb-4">{t('challenge.new')}</h2>

                <div className="space-y-6 mb-8">
                    <div>
                        <label className="block text-[10px] font-black text-slate-500 uppercase tracking-widest mb-3">{t('challenge.stake')}</label>
                        <div className="relative">
                            <input
                                type="text"
                                inputMode="decimal"
                                value={stake}
                                onChange={e => setStake(e.target.value)}
                                className="w-full p-4 bg-slate-800 border border-slate-700 rounded-xl focus:border-kaspa-primary outline-none transition-colors font-bold text-lg pr-16"
                                placeholder="50"
                            />
                            <span className="absolute right-4 top-1/2 -translate-y-1/2 text-slate-500 font-black text-xs tracking-widest">KAS</span>
                        </div>
                    </div>

                    <div>
                        <label className="block text-[10px] font-black text-slate-500 uppercase tracking-widest mb-3">{t('challenge.mode')}</label>
                        <div className="grid grid-cols-2 gap-4">
                            {['BO1', 'BO3'].map((m) => (
                                <button
                                    key={m}
                                    type="button"
                                    onClick={() => setMode(m as MatchMode)}
                                    className={`py-3 rounded-xl font-black uppercase tracking-tighter transition-all border ${mode === m
                                        ? 'bg-kaspa-primary/10 border-kaspa-primary text-kaspa-primary'
                                        : 'bg-slate-800 border-slate-700 text-slate-400 hover:border-slate-500'
                                        }`}
                                >
                                    {m}
                                </button>
                            ))}
                        </div>
                    </div>
                </div>

                <div className="flex gap-4">
                    <button
                        type="button"
                        onClick={() => setOpen(false)}
                        className="flex-1 py-4 bg-slate-800 hover:bg-slate-700 text-slate-300 font-bold rounded-xl transition-all"
                    >
                        {t('challenge.cancel')}
                    </button>
                    <button
                        type="submit"
                        disabled={isCreating}
                        className="flex-[2] bg-kaspa-primary hover:bg-kaspa-secondary text-kaspa-dark py-4 rounded-xl font-black uppercase tracking-tighter shadow-lg shadow-kaspa-primary/10 transition-all active:scale-95 disabled:opacity-50"
                    >
                        {isCreating ? t('challenge.creating') : t('challenge.create_submit')}
                    </button>
                </div>
            </form>
        </div>
    );
};
