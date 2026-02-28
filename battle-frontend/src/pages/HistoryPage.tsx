import React, { useEffect } from 'react';
import { useLobbyStore } from '../stores/useLobbyStore';

export const HistoryPage: React.FC = () => {
    const { history, setHistory } = useLobbyStore();

    useEffect(() => {
        fetch('/api/history').then(r => r.json()).then(data => setHistory(data || [])).catch(console.error);
    }, [setHistory]);

    return (
        <div className="container mx-auto p-4 max-w-5xl text-white">
            <h1 className="text-3xl font-bold mb-8">Verlauf</h1>
            <div className="flex flex-col gap-3">
                {history.length === 0 && <p>Bisher keine Matches abgeschlossen.</p>}
                {history.map(m => (
                    <div key={m.id} className="bg-slate-800 p-4 rounded border border-slate-700 flex justify-between">
                        <span>{m.game_id} | {m.match_mode} | {m.wager_amount_sompi / 100000000} KAS</span>
                        {m.payout_tx_hash && <a href={`https://explorer.kaspa.org/txs/${m.payout_tx_hash}`} target="_blank" rel="noreferrer" className="text-blue-400">TX Link</a>}
                    </div>
                ))}
            </div>
        </div>
    );
};
