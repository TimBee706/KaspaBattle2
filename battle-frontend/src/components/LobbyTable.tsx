import React, { useState } from 'react';
import type { BattleMatch } from '../api/types';
import { useAuthStore } from '../stores/useAuthStore';
import { useWallet } from '../hooks/useWallet';

export const LobbyTable: React.FC<{ matches: BattleMatch[], title: string, isMyLobbies?: boolean }> = ({ matches, title, isMyLobbies }) => {
    const { user } = useAuthStore();
    const kaspaAddress = user?.kas_address || null;
    const faceitId = user?.faceit_id || null;
    const { signAndSendDeposit } = useWallet();
    const [loading, setLoading] = useState<string | null>(null);

    const join = async (id: string, stake: number) => {
        setLoading(id);
        try {
            const res = await fetch(`/api/challenges/${id}/join`, { method: 'POST' });
            if (!res.ok) throw new Error("Join failed");
            await signAndSendDeposit(id, stake);
        } catch (e) { console.error(e); }
        finally { setLoading(null); }
    };

    return (
        <div className="mb-8 p-4 bg-slate-800 rounded-lg">
            <h2 className="text-xl font-bold mb-4 text-emerald-400">{title}</h2>
            <div className="flex flex-col gap-3">
                {matches.length === 0 && <p className="text-slate-400">Keine Einträge.</p>}
                {matches.map(m => (
                    <div key={m.id} className="bg-slate-900 border border-slate-700 p-4 rounded flex justify-between items-center text-white">
                        <span>{m.game_id} | {m.match_mode} <span className="text-gray-400 text-sm ml-2">[{m.status}]</span></span>
                        <div className="flex items-center gap-4">
                            <span className="font-bold text-emerald-400">{m.wager_amount_sompi / 100000000} KAS</span>
                            {!isMyLobbies && m.status === 'OPEN' && (
                                <button
                                    onClick={() => join(m.id, m.wager_amount_sompi / 100000000)}
                                    disabled={loading === m.id || !kaspaAddress || !faceitId}
                                    className="bg-emerald-600 px-4 py-2 rounded font-bold disabled:opacity-50">
                                    {loading === m.id ? 'Lädt...' : 'Join'}
                                </button>
                            )}
                        </div>
                    </div>
                ))}
            </div>
            {!isMyLobbies && !kaspaAddress && <p className="text-red-400 mt-2 text-sm">Wallet verbinden zum Beitreten!</p>}
            {!isMyLobbies && kaspaAddress && !faceitId && <p className="text-yellow-400 mt-2 text-sm">FACEIT verbinden zum Beitreten!</p>}
        </div>
    );
};
