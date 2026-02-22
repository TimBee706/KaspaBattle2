import { useAuthStore } from '../stores/useAuthStore';
import { formatKas, shortenAddress } from '../utils/format';

export function ProfilePage() {
    const { user } = useAuthStore();

    if (!user) return null;

    const winRate = user.total_matches > 0 ? (user.wins / user.total_matches) * 100 : 0;

    return (
        <div className="max-w-4xl mx-auto space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-500">
            {/* Profile Header */}
            <div className="card flex flex-col md:flex-row items-center gap-8 p-10 bg-gradient-to-br from-kaspa-card to-kaspa-dark">
                <div className="relative">
                    {user.faceit_avatar ? (
                        <img src={user.faceit_avatar} alt="" className="w-32 h-32 rounded-full border-4 border-kaspa-primary shadow-2xl" />
                    ) : (
                        <div className="w-32 h-32 rounded-full bg-kaspa-border flex items-center justify-center text-5xl font-black border-4 border-kaspa-primary text-kaspa-primary">
                            {user.faceit_nickname[0].toUpperCase()}
                        </div>
                    )}
                    <div className="absolute -bottom-2 left-1/2 -translate-x-1/2 px-4 py-1 bg-kaspa-primary text-kaspa-dark text-[10px] font-black rounded-full uppercase tracking-tighter">
                        LVL 10
                    </div>
                </div>

                <div className="flex-1 text-center md:text-left">
                    <h1 className="text-4xl font-black tracking-tighter mb-1">{user.faceit_nickname}</h1>
                    <p className="text-gray-500 font-mono text-sm mb-6">{shortenAddress(user.kas_address)}</p>

                    <div className="flex flex-wrap justify-center md:justify-start gap-4">
                        <div className="px-3 py-1 bg-kaspa-border rounded border border-gray-700 text-xs font-bold text-gray-400">
                            ID: {user.faceit_id}
                        </div>
                        <div className="px-3 py-1 bg-orange-600/20 rounded border border-orange-500/30 text-xs font-bold text-orange-400">
                            VERIFIED PLAYER
                        </div>
                    </div>
                </div>
            </div>

            {/* Stats Grid */}
            <div className="grid grid-cols-1 md:grid-cols-4 gap-6">
                <div className="card text-center p-8">
                    <span className="text-[10px] font-black text-gray-500 uppercase tracking-widest block mb-2">Gesamt Matches</span>
                    <span className="text-4xl font-black text-white">{user.total_matches}</span>
                </div>
                <div className="card text-center p-8 border-b-4 border-b-green-500">
                    <span className="text-[10px] font-black text-gray-500 uppercase tracking-widest block mb-2">Siege</span>
                    <span className="text-4xl font-black text-green-500">{user.wins}</span>
                </div>
                <div className="card text-center p-8 border-b-4 border-b-red-500">
                    <span className="text-[10px] font-black text-gray-500 uppercase tracking-widest block mb-2">Niederlagen</span>
                    <span className="text-4xl font-black text-red-500">{user.losses}</span>
                </div>
                <div className="card text-center p-8 border-b-4 border-b-kaspa-primary">
                    <span className="text-[10px] font-black text-gray-500 uppercase tracking-widest block mb-2">Win Rate</span>
                    <span className="text-4xl font-black text-kaspa-primary">{winRate.toFixed(1)}%</span>
                </div>
            </div>

            {/* Financials */}
            <div className="card grid grid-cols-1 md:grid-cols-2 gap-8 p-10">
                <div>
                    <h3 className="text-sm font-black text-gray-500 uppercase tracking-widest mb-6 border-l-4 border-kaspa-primary pl-4">Gewinn Statistik</h3>
                    <div className="space-y-4">
                        <div className="flex justify-between items-center">
                            <span className="text-gray-400 text-sm">Gesamt umgesetzt</span>
                            <span className="font-bold text-white">{formatKas(user.total_wagered_sompi)} KAS</span>
                        </div>
                        <div className="flex justify-between items-center">
                            <span className="text-gray-400 text-sm">Gesamt gewonnen</span>
                            <span className="font-bold text-kaspa-primary">+{formatKas(user.total_won_sompi)} KAS</span>
                        </div>
                        <div className="h-px bg-kaspa-border" />
                        <div className="flex justify-between items-center">
                            <span className="text-gray-400 text-sm font-bold">Netto Profit</span>
                            <span className="text-lg font-black text-emerald-500">+{(formatKas(user.total_won_sompi - (user.total_wagered_sompi / 2)))} KAS</span>
                        </div>
                    </div>
                </div>

                <div className="flex items-center justify-center p-6 bg-kaspa-dark rounded-2xl border border-kaspa-border border-dashed">
                    <div className="text-center">
                        <p className="text-[10px] text-gray-500 font-black uppercase mb-2">Player Rank</p>
                        <div className="text-6xl mb-2">🥈</div>
                        <p className="text-xl font-bold italic tracking-tighter">SILVER COMMANDER</p>
                        <p className="text-[10px] text-gray-600 mt-2">Nächstes Level bei 10 Siegen</p>
                    </div>
                </div>
            </div>
        </div>
    );
}
