import { Link } from 'react-router-dom';
import { useAuthStore } from '../stores/useAuthStore';
import { startFaceitLogin } from '../api/auth';
import { SUPPORTED_GAMES } from '../config/constants';

export function LandingPage() {
    const { isAuthenticated } = useAuthStore();

    return (
        <div className="animate-in fade-in slide-in-from-bottom-4 duration-1000">
            {/* Hero Section */}
            <section className="py-20 text-center relative overflow-hidden">
                <div className="absolute top-0 left-1/2 -translate-x-1/2 w-[800px] h-[400px] bg-kaspa-primary/10 blur-[120px] rounded-full -z-10" />

                <div className="inline-flex items-center gap-2 px-3 py-1 bg-kaspa-primary/10 border border-kaspa-primary/20 rounded-full text-kaspa-primary text-[10px] font-black tracking-widest uppercase mb-8">
                    <span className="relative flex h-2 w-2">
                        <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-kaspa-primary opacity-75"></span>
                        <span className="relative inline-flex rounded-full h-2 w-2 bg-kaspa-primary"></span>
                    </span>
                    Beitritt Phase 1: OPEN BETA
                </div>

                <h1 className="text-5xl md:text-7xl font-black tracking-tighter mb-6 leading-[0.9]">
                    PLAY TO <span className="text-kaspa-primary">WIN</span><br />
                    ON <span className="underline decoration-kaspa-primary decoration-8">KASPA</span>
                </h1>

                <p className="text-lg md:text-xl text-gray-400 max-w-2xl mx-auto mb-10 font-medium">
                    Die weltweit erste dezentrale Gaming-Wager-Plattform auf Kaspa.
                    Sicher, blitzschnell und 100% Non-Custodial.
                </p>

                <div className="flex flex-col sm:flex-row items-center justify-center gap-4">
                    {isAuthenticated ? (
                        <Link to="/lobby" className="btn-primary px-10 py-4 text-lg">
                            JETZT SPIELEN
                        </Link>
                    ) : (
                        <button onClick={startFaceitLogin} className="btn-primary bg-orange-600 hover:bg-orange-500 px-10 py-4 text-lg">
                            LOGIN MIT FACEIT
                        </button>
                    )}
                    <a href="#how" className="px-10 py-4 bg-kaspa-card border border-kaspa-border hover:bg-kaspa-border text-white rounded-lg font-bold text-lg transition-all">
                        WIE ES FUNKTIONIERT
                    </a>
                </div>

                <div className="mt-20 flex justify-center items-center gap-8 md:gap-16 opacity-40 grayscale hover:grayscale-0 transition-all duration-500">
                    {SUPPORTED_GAMES.map(game => (
                        <div key={game.id} className="flex flex-col items-center gap-2">
                            <span className="text-4xl">{game.icon}</span>
                            <span className="text-[10px] font-black uppercase tracking-tighter">{game.name}</span>
                        </div>
                    ))}
                </div>
            </section>

            {/* Features */}
            <section className="py-20 grid grid-cols-1 md:grid-cols-3 gap-8">
                <div className="card border-t-4 border-t-kaspa-primary">
                    <div className="text-4xl mb-4 text-kaspa-primary">🛡️</div>
                    <h3 className="text-xl font-bold mb-2">Escrow Smart Contracts</h3>
                    <p className="text-gray-400 text-sm leading-relaxed">Dein Geld ist sicher in einer Multi-Sig-ähnlichen Escrow-Adresse auf der Kaspa-Blockchain. Niemand kann darauf zugreifen außer dem Gewinner.</p>
                </div>
                <div className="card border-t-4 border-t-blue-500">
                    <div className="text-4xl mb-4 text-blue-500">⚡</div>
                    <h3 className="text-xl font-bold mb-2">Kaspa Performance</h3>
                    <p className="text-gray-400 text-sm leading-relaxed">Nutze die Geschwindigkeit von Kaspa (10 BPS) für sofortige Einzahlungen und Auszahlungen ohne Wartezeit.</p>
                </div>
                <div className="card border-t-4 border-t-yellow-500">
                    <div className="text-4xl mb-4 text-yellow-500">🎯</div>
                    <h3 className="text-xl font-bold mb-2">Automatisierte Oracles</h3>
                    <p className="text-gray-400 text-sm leading-relaxed">Unsere Oracle-Service fragt FACEIT APIs direkt ab und verteilt den Gewinn automatisch an den Sieger - manipulationssicher.</p>
                </div>
            </section>

            {/* How it works */}
            <section id="how" className="py-20 border-t border-kaspa-border">
                <h2 className="text-3xl font-black text-center mb-16 underline decoration-kaspa-primary decoration-4 underline-offset-8">DER PROZESS</h2>
                <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
                    {[
                        { step: "01", title: "Challenge", text: "Erstelle eine Challenge oder nimm eine an." },
                        { step: "02", title: "Funding", text: "Beide Spieler zahlen den Einsatz in den Escrow." },
                        { step: "03", title: "Battle", text: "Spiele dein Match auf FACEIT wie gewohnt." },
                        { step: "04", title: "Payout", text: "Der Oracle zahlt den Pott automatisch aus." }
                    ].map((item, i) => (
                        <div key={i} className="relative p-6 bg-kaspa-card/50 rounded-2xl border border-kaspa-border overflow-hidden">
                            <span className="absolute -top-4 -right-2 text-7xl font-black opacity-5 italic text-kaspa-primary">{item.step}</span>
                            <h4 className="text-lg font-bold mb-2 text-kaspa-primary">{item.title}</h4>
                            <p className="text-gray-500 text-xs leading-relaxed">{item.text}</p>
                        </div>
                    ))}
                </div>
            </section>
        </div>
    );
}
