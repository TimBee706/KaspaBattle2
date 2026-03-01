import React, { useState } from 'react';
import { useAuthStore } from '../stores/useAuthStore';
import { kasToSompi } from '../utils/format';
import apiClient from '../api/client';
import { useTranslation } from 'react-i18next';

export const ChallengeModal: React.FC = () => {
    const [open, setOpen] = useState(false);
    const [stake, setStake] = useState<number | string>(50);
    const [mode, setMode] = useState<'BO1' | 'BO3'>('BO1');
    const { user } = useAuthStore();
    const kaspaAddress = user?.kaspa_address || null;
    const faceitId = user?.faceit_id || null;
    const { t } = useTranslation();

    const submit = async (e: React.FormEvent) => {
        e.preventDefault();
        try {
            await apiClient.post('/challenges', {
                game_id: 'CS2',
                stake_kas: kasToSompi(stake),
                mode: mode
            });
            setOpen(false);
        } catch (e) { console.error(e); }
    };

    if (!open) return (
        <button onClick={() => setOpen(true)} disabled={!kaspaAddress || !faceitId} className="bg-blue-600 px-4 py-2 text-white rounded font-bold disabled:opacity-50">
            {t('challenge.create')}
        </button>
    );

    return (
        <div className="fixed inset-0 bg-black/80 flex items-center justify-center z-50 text-white">
            <form onSubmit={submit} className="bg-slate-900 p-6 rounded w-96 border border-slate-700">
                <h2 className="text-xl font-bold mb-4">{t('challenge.new')}</h2>
                <div className="mb-4">
                    <label className="block mb-2">{t('challenge.stake')}</label>
                    <input
                        type="text"
                        inputMode="decimal"
                        value={stake}
                        onChange={e => setStake(e.target.value)}
                        className="w-full p-2 bg-slate-800 rounded"
                        placeholder="z.B. 49.99 oder 49,99"
                    />
                </div>
                <div className="mb-6">
                    <label className="block mb-2">{t('challenge.mode')}</label>
                    <select value={mode} onChange={e => setMode(e.target.value as any)} className="w-full p-2 bg-slate-800 rounded">
                        <option value="BO1">Best of 1</option>
                        <option value="BO3">Best of 3</option>
                    </select>
                </div>
                <div className="flex justify-end gap-2">
                    <button type="button" onClick={() => setOpen(false)} className="px-4 py-2">{t('challenge.cancel')}</button>
                    <button type="submit" className="bg-blue-600 px-4 py-2 rounded">{t('challenge.create_submit')}</button>
                </div>
            </form>
        </div>
    );
};
