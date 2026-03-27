import { Link } from 'react-router-dom';
import { useAuthStore } from '../stores/useAuthStore';
import { startFaceitLogin, startFaceitLink } from '../api/auth';
import { useTranslation, Trans } from 'react-i18next';

export function LandingPage() {
    const { isAuthenticated, isFullyConnected } = useAuthStore();
    const { t } = useTranslation();

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
                    {t('hero.beta_tag')}
                </div>

                <h1 className="text-5xl md:text-7xl font-black tracking-tighter mb-6 leading-[0.9] uppercase">
                    <Trans i18nKey="hero.title">
                        PLAY TO WIN <span className="text-kaspa-primary">KASPA</span>
                    </Trans>
                </h1>

                <p className="text-lg md:text-xl text-gray-400 max-w-2xl mx-auto mb-10 font-medium">
                    {t('hero.subtitle')}
                </p>


            </section>

            {/* Was ist KaspaBattle? */}
            <section className="py-20 border-t border-kaspa-border relative">
                <h2 className="text-3xl md:text-4xl font-black text-center mb-10 underline decoration-kaspa-primary decoration-4 underline-offset-8">{t('sections.what_is')}</h2>
                <div className="max-w-4xl mx-auto text-center bg-kaspa-card/40 p-10 rounded-3xl border border-kaspa-border backdrop-blur-sm relative overflow-hidden">
                    <div className="absolute -inset-1 bg-gradient-to-r from-kaspa-primary/10 via-transparent to-kaspa-primary/10 blur-xl opacity-50 -z-10" />
                    <p className="text-lg md:text-xl text-gray-300 leading-relaxed font-medium">
                        <Trans i18nKey="sections.what_is_content">
                            KaspaBattle ist eine dezentrale <strong className="text-white font-bold">Peer‑to‑Peer Gaming-Wager-Plattform</strong> auf Kaspa.
                            Zwei Spieler zahlen KAS in einen sicheren Smart-Contract-Escrow ein. Nach einem Match (z.B. CS2 auf FACEIT) wertet unser automatisiertes Oracle das Ergebnis aus und der Gewinner erhält automatisch den Payout.
                        </Trans>
                    </p>
                </div>
            </section>

            {/* 2. So funktioniert ein Kaspa Battle (Umbau zur vertikalen Nummern-Liste) */}
            <section id="how" className="py-20 border-t border-kaspa-border">
                <h2 className="text-3xl md:text-4xl font-black text-center mb-16 underline decoration-kaspa-primary decoration-4 underline-offset-8">{t('sections.how_it_works')}</h2>
                <div className="max-w-3xl mx-auto flex flex-col gap-4 px-4">
                    {[
                        { step: "1", icon: "🔗", title: t('sections.how_it_works_steps.1') },
                        { step: "2", icon: "⚔️", title: t('sections.how_it_works_steps.2') },
                        { step: "3", icon: "💰", title: t('sections.how_it_works_steps.3') },
                        { step: "4", icon: "🎮", title: t('sections.how_it_works_steps.4') },
                        { step: "5", icon: "📊", title: t('sections.how_it_works_steps.5') },
                        { step: "6", icon: "💸", title: t('sections.how_it_works_steps.6') }
                    ].map((item, i) => (
                        <div key={i} className="flex items-center gap-6 p-6 bg-kaspa-card/50 rounded-2xl border border-kaspa-border hover:bg-kaspa-card hover:border-kaspa-primary/30 transition-all">
                            <div className="w-14 h-14 shrink-0 rounded-full bg-kaspa-primary/10 text-kaspa-primary flex items-center justify-center font-black border border-kaspa-primary/20 text-2xl relative">
                                {item.step}
                                <span className="absolute -bottom-2 -right-2 text-xl">{item.icon}</span>
                            </div>
                            <h4 className="text-lg md:text-xl font-bold text-white tracking-wide">{item.title}</h4>
                        </div>
                    ))}
                </div>
            </section>

            {/* Was schon implementiert ist (Beta) */}
            <section className="py-20 border-t border-kaspa-border">
                <h2 className="text-3xl md:text-4xl font-black text-center mb-16 underline decoration-kaspa-primary decoration-4 underline-offset-8">{t('sections.implemented_features')}</h2>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-8 max-w-5xl mx-auto">
                    <div className="card border-l-4 border-l-kaspa-primary bg-kaspa-card/60 p-8 rounded-2xl relative overflow-hidden flex flex-col">
                        <div className="absolute top-0 right-0 w-32 h-32 bg-kaspa-primary/10 blur-[50px] -z-10" />
                        <div className="flex items-center gap-4 mb-8">
                            <div className="w-12 h-12 rounded-xl bg-kaspa-primary/20 flex items-center justify-center text-2xl">⚡</div>
                            <h3 className="text-2xl font-black text-white">{t('sections.already_live')}</h3>
                        </div>
                        <ul className="space-y-4 text-gray-300 font-medium flex-1">
                            <li className="flex items-start gap-3"><span className="text-kaspa-primary mt-1 text-xl">✓</span> {t('sections.live_features.faceit')}</li>
                            <li className="flex items-start gap-3"><span className="text-kaspa-primary mt-1 text-xl">✓</span> {t('sections.live_features.lobby')}</li>
                            <li className="flex items-start gap-3"><span className="text-kaspa-primary mt-1 text-xl">✓</span> {t('sections.live_features.escrow')}</li>
                            <li className="flex items-start gap-3"><span className="text-kaspa-primary mt-1 text-xl">✓</span> {t('sections.live_features.payout')}</li>
                            <li className="flex items-start gap-3"><span className="text-kaspa-primary mt-1 text-xl">✓</span> {t('sections.live_features.history')}</li>
                        </ul>
                    </div>
                    <div className="card border-l-4 border-l-blue-500 bg-kaspa-card/60 p-8 rounded-2xl relative overflow-hidden flex flex-col">
                        <div className="absolute top-0 right-0 w-32 h-32 bg-blue-500/10 blur-[50px] -z-10" />
                        <div className="flex items-center gap-4 mb-8">
                            <div className="w-12 h-12 rounded-xl bg-blue-500/20 flex items-center justify-center text-2xl">🚀</div>
                            <h3 className="text-2xl font-black text-white">{t('sections.in_progress')}</h3>
                        </div>
                        <ul className="space-y-4 text-gray-400 font-medium flex-1">
                            <li className="flex items-start gap-3"><span className="text-blue-500 mt-1 text-lg">→</span> {t('sections.next_steps.more_games')}</li>
                            <li className="flex items-start gap-3"><span className="text-blue-500 mt-1 text-lg">→</span> {t('sections.next_steps.smart_contracts')}</li>
                            <li className="flex items-start gap-3"><span className="text-blue-500 mt-1 text-lg">→</span> {t('sections.next_steps.oracle')}</li>
                            <li className="flex items-start gap-3"><span className="text-blue-500 mt-1 text-lg">→</span> {t('sections.next_steps.tournament')}</li>
                        </ul>
                    </div>
                </div>
            </section>

            {/* Warum Kaspa? */}
            <section className="py-20 border-t border-kaspa-border">
                <h2 className="text-3xl md:text-4xl font-black text-center mb-16 underline decoration-kaspa-primary decoration-4 underline-offset-8">{t('sections.why_kaspa')}</h2>
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
                    {[
                        { icon: "⚡", title: t('sections.features.finality.title'), text: t('sections.features.finality.text') },
                        { icon: "🏎️", title: t('sections.features.blocks.title'), text: t('sections.features.blocks.text') },
                        { icon: "💸", title: t('sections.features.fees.title'), text: t('sections.features.fees.text') },
                        { icon: "🛡️", title: t('sections.features.security.title'), text: t('sections.features.security.text') }
                    ].map((feature, i) => (
                        <div key={i} className="card bg-kaspa-card/40 hover:bg-kaspa-card/80 border border-kaspa-border hover:border-kaspa-primary/30 transition-all p-8 text-center rounded-2xl hover:-translate-y-1 duration-300">
                            <div className="text-5xl mb-6 mx-auto bg-kaspa-dark w-20 h-20 rounded-full flex items-center justify-center border border-kaspa-border shadow-inner">{feature.icon}</div>
                            <h3 className="text-lg font-bold mb-3 text-white">{feature.title}</h3>
                            <p className="text-gray-400 text-sm leading-relaxed">{feature.text}</p>
                        </div>
                    ))}
                </div>
            </section>

            {/* 3. Die Zukunft des eSports (Neuer Abschnitt am Ende / CTA) */}
            <section className="py-24 border-t border-kaspa-border text-center relative overflow-hidden">
                <div className="absolute inset-0 bg-gradient-to-t from-kaspa-primary/10 to-transparent -z-10" />
                <h2 className="text-4xl md:text-6xl font-black mb-6 uppercase tracking-tighter">{t('sections.future')}</h2>
                <p className="text-xl text-kaspa-primary mb-4 max-w-2xl mx-auto font-bold tracking-wide">
                    {t('sections.future_subtitle')}
                </p>

                {/* Kaspa Battle Logo */}
                <div className="mb-4 flex justify-center relative">
                    <span className="relative z-10 flex items-center justify-center">
                        <img src="/kaspa-battle_logo.svg" alt="Kaspa Battle Logo" className="w-64 h-64 md:w-80 md:h-80 object-contain drop-shadow-[0_0_30px_rgba(112,199,186,0.3)]" />
                    </span>
                </div>

                <h3 className="text-3xl font-black mb-10 text-white">{t('sections.ready')}</h3>

                {/* Vertikal gestackte Buttons (Stacked) */}
                <div className="flex flex-col items-center justify-center gap-4 max-w-sm mx-auto">
                    {(isAuthenticated && isFullyConnected) ? (
                        <Link to="/lobby" className="btn-primary w-full px-12 py-5 text-xl shadow-[0_0_30px_rgba(112,199,186,0.3)] hover:shadow-[0_0_50px_rgba(112,199,186,0.5)] transition-shadow text-kaspa-dark font-black flex items-center justify-center">
                            {t('navigation.back_to_lobby')}
                        </Link>
                    ) : (
                        <>
                            <Link to="/wallet/import" className="btn-primary w-full px-10 py-5 text-lg bg-kaspa-primary hover:bg-kaspa-primary/80 flex items-center gap-3 text-kaspa-dark font-black">
                                🔌 {t('wallet.connect')}
                            </Link>
                            <button onClick={() => (isAuthenticated ? startFaceitLink() : startFaceitLogin())} className="btn-primary w-full bg-orange-600 hover:bg-orange-500 px-10 py-5 text-lg flex items-center gap-3 text-white font-black">
                                🎮 {t('navigation.faceit_login')}
                            </button>
                        </>
                    )}
                </div>
            </section>
        </div>
    );
}
