import { CreateChallengeForm } from '../components/match/CreateChallengeForm';

export function CreateMatchPage() {
    return (
        <div className="py-10 animate-in fade-in slide-in-from-top-4 duration-500">
            <div className="max-w-xl mx-auto mb-10 text-center">
                <h1 className="text-4xl font-black mb-4 tracking-tight">FORDERE JEMANDEN HERAUS</h1>
                <p className="text-gray-400">
                    Wähle dein Spiel, deinen Einsatz und den Modus. Deine Challenge wird in der Lobby für alle Spieler sichtbar sein.
                </p>
            </div>

            <CreateChallengeForm />

            <div className="max-w-lg mx-auto mt-12 grid grid-cols-1 md:grid-cols-2 gap-6 text-center">
                <div className="p-4 bg-kaspa-card/30 rounded-xl border border-kaspa-border">
                    <span className="text-kaspa-primary text-xl block mb-2">⚡</span>
                    <p className="text-[10px] text-gray-500 uppercase tracking-widest font-bold">Instante Erstellung</p>
                </div>
                <div className="p-4 bg-kaspa-card/30 rounded-xl border border-kaspa-border">
                    <span className="text-kaspa-primary text-xl block mb-2">🔒</span>
                    <p className="text-[10px] text-gray-500 uppercase tracking-widest font-bold">100% Escrow Schutz</p>
                </div>
            </div>
        </div>
    );
}
