import { Link } from 'react-router-dom';
import { useAuthStore } from '../stores/useAuthStore';
import { startFaceitLogin, startFaceitLink } from '../api/auth';
import { useTranslation, Trans } from 'react-i18next';
import { Icon } from '../components/Icon';
import { PublicBetaBadge } from '../components/common/PublicBetaBadge';
import { SectionHeading } from '../components/common/SectionHeading';
import { SocialLinks } from '../components/common/SocialLinks';
import { ContactCard } from '../components/common/ContactCard';
import { RecruitmentSection } from '../components/landing/RecruitmentSection';
import { publicLinks } from '../config/publicLinks';
import { KaspaCoin } from '../components/native/ConnectFourCell';
import { useNativeGamesEnabled } from '../hooks/useNativeGamesEnabled';
import { NodeHelpNotice } from '../components/freeplay/NodeHelpNotice';

export function LandingPage() {
    const { isAuthenticated, isFullyConnected, testMode } = useAuthStore();
    const { t } = useTranslation();
    const nativeEnabled = useNativeGamesEnabled();

    return (
        <div className="animate-fade-in-up">
            {/* Hero Section */}
            <section className="py-20 text-center relative overflow-hidden">
                <div aria-hidden="true" className="absolute top-0 left-1/2 -translate-x-1/2 w-[800px] h-[400px] bg-kaspa-primary/10 blur-[120px] rounded-full -z-10" />

                <div className="flex flex-wrap items-center justify-center gap-3 mb-8">
                    <PublicBetaBadge />
                    <span className="inline-flex items-center gap-1.5 rounded-full bg-[#FF5500]/10 border border-[#FF5500]/25 text-[#FF5500] font-black uppercase tracking-widest px-3 py-1 text-[10px]">
                        <Icon name="link" className="w-3 h-3" />
                        {t('hero.faceit_badge')}
                    </span>
                    <span className="inline-flex items-center gap-1.5 rounded-full bg-kaspa-primary/10 border border-kaspa-primary/25 text-kaspa-primary font-black uppercase tracking-widest text-2xs px-3 py-1">
                        <Icon name="bolt" className="w-3 h-3" />
                        {t('hero.browser_badge')}
                    </span>
                </div>

                <h1 className="text-5xl md:text-7xl font-black tracking-tighter mb-6 leading-[0.95] uppercase max-w-4xl mx-auto">
                    <Trans i18nKey="hero.title">
                        Competitive gaming, settled on <span className="text-kaspa-primary">Kaspa</span>.
                    </Trans>
                </h1>

                <p className="text-lg md:text-xl text-gray-400 max-w-2xl mx-auto mb-10 font-medium">
                    {t('hero.subtitle')}
                </p>

                <div className="flex flex-col sm:flex-row items-center justify-center gap-4 mb-10">
                    <Link
                        to="/lobby"
                        className="btn-primary w-full sm:w-auto px-10 py-4 text-base shadow-glow-primary hover:shadow-glow-primary-lg transition-shadow text-kaspa-dark font-black inline-flex items-center justify-center gap-2"
                    >
                        {t('hero.cta_primary')}
                        <Icon name="chevron-right" className="w-4 h-4" />
                    </Link>
                    <a
                        href="#get-involved"
                        className="glass-button w-full sm:w-auto px-10 py-4 text-base font-black inline-flex items-center justify-center gap-2"
                    >
                        {t('hero.cta_secondary')}
                    </a>
                    <a
                        href={publicLinks.githubRepository}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="text-sm font-semibold text-gray-400 hover:text-kaspa-primary inline-flex items-center gap-1.5 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-kaspa-primary rounded"
                    >
                        <Icon name="github" className="w-4 h-4" />
                        {t('hero.cta_github')}
                    </a>
                </div>

                <div className="flex flex-wrap items-center justify-center gap-x-6 gap-y-2 text-xs text-gray-500 font-semibold">
                    <span className="inline-flex items-center gap-1.5">
                        <Icon name="shield" className="w-3.5 h-3.5 text-kaspa-primary/70" />
                        {t('hero.status.testnet_only')}
                    </span>
                    <span className="inline-flex items-center gap-1.5">
                        <Icon name="bolt" className="w-3.5 h-3.5 text-kaspa-primary/70" />
                        {t('hero.status.active_development')}
                    </span>
                    <span className="inline-flex items-center gap-1.5">
                        <Icon name="users" className="w-3.5 h-3.5 text-kaspa-primary/70" />
                        {t('hero.status.open_for_testers')}
                    </span>
                    <span className="inline-flex items-center gap-1.5">
                        <Icon name="code" className="w-3.5 h-3.5 text-kaspa-primary/70" />
                        {t('hero.status.contributors_welcome')}
                    </span>
                </div>
            </section>

            {/* Free Play: live now, no wallet / FACEIT / node required – plus the node call-for-help */}
            <section id="free-play" className="pb-16 px-4">
                <div className="mx-auto mb-6 max-w-3xl text-center">
                    <p className="text-lg font-semibold text-gray-300">{t('freeplay.landing_pitch')}</p>
                    <div className="mt-5 flex flex-col items-center justify-center gap-3 sm:flex-row">
                        {isAuthenticated ? (
                            <Link to="/free-play" className="btn-primary flex h-12 items-center justify-center px-8 font-black">{t('freeplay.cta_play')}</Link>
                        ) : (
                            <>
                                <Link to="/register" className="btn-primary flex h-12 items-center justify-center px-8 font-black">{t('freeplay.register_cta')}</Link>
                                <Link to="/login" className="glass-button flex h-12 items-center justify-center px-8 font-bold">{t('auth.login.submit')}</Link>
                            </>
                        )}
                    </div>
                </div>
                <NodeHelpNotice />
            </section>

            {/* Was ist KaspaBattle? */}
            <section className="py-20 border-t border-kaspa-border">
                <SectionHeading eyebrow={t('sections.what_is_eyebrow')} title={t('sections.what_is')} />
                <div className="max-w-3xl mx-auto px-4 text-center">
                    <p className="text-lg md:text-xl text-gray-300 leading-relaxed font-medium mb-6">
                        <Trans i18nKey="sections.what_is_content">
                            KaspaBattle ist eine Peer-to-Peer-Plattform für kompetitives Gaming. Spiele Browser Games direkt auf KaspaBattle oder tritt in <span className="text-[#FF5500] font-semibold">FACEIT-Matches</span> an – beides wird mit <span className="text-kaspa-primary font-semibold">Kaspa-basiertem Escrow und Payout</span> abgewickelt.
                        </Trans>
                    </p>
                    <p className="text-base text-gray-400 leading-relaxed">
                        {t('sections.what_is_integration')}
                    </p>
                </div>
            </section>

            {/* Two ways to play: Browser Games (native) and FACEIT Games */}
            <section id="games" className="py-20 border-t border-kaspa-border">
                <SectionHeading
                    eyebrow={t('sections.two_ways.eyebrow')}
                    title={t('sections.two_ways.title')}
                    subtitle={t('sections.two_ways.subtitle')}
                />
                <div className="max-w-5xl mx-auto px-4 grid grid-cols-1 md:grid-cols-2 gap-6">
                    <article aria-labelledby="games-browser-title" data-testid="landing-browser-games" className="glass-panel rounded-2xl border border-kaspa-primary/25 p-6 md:p-8 flex flex-col">
                        <div className="flex items-center gap-2 mb-4">
                            <KaspaCoin color="blue" className="w-8 h-8" />
                            <KaspaCoin color="red" className="w-8 h-8" />
                        </div>
                        <p className="text-xs text-kaspa-primary/80 font-semibold uppercase tracking-wide mb-2">{t('sections.two_ways.browser.tag')}</p>
                        <h3 id="games-browser-title" className="text-2xl font-black text-white mb-3">{t('sections.two_ways.browser.title')}</h3>
                        <p className="text-gray-400 text-sm leading-relaxed mb-4">{t('sections.two_ways.browser.text')}</p>
                        <ul className="space-y-2 text-sm text-gray-300 mb-6">
                            {(['p1', 'p2', 'p3'] as const).map((k) => (
                                <li key={k} className="flex items-start gap-2">
                                    <Icon name="check" className="w-4 h-4 mt-0.5 shrink-0 text-kaspa-primary" />
                                    {t(`sections.two_ways.browser.${k}`)}
                                </li>
                            ))}
                        </ul>
                        <div className="mt-auto">
                            {nativeEnabled ? (
                                <Link to="/lobby/create?provider=native" className="btn-primary inline-flex items-center gap-2 px-6 py-3 text-kaspa-dark font-black">
                                    {t('sections.two_ways.browser.cta')}
                                    <Icon name="chevron-right" className="w-4 h-4" />
                                </Link>
                            ) : (
                                <span className="text-2xs font-bold uppercase tracking-widest text-gray-500">{t('sections.two_ways.browser.soon')}</span>
                            )}
                        </div>
                    </article>

                    <article aria-labelledby="games-faceit-title" data-testid="landing-faceit-games" className="glass-panel rounded-2xl border border-[#FF5500]/25 p-6 md:p-8 flex flex-col">
                        <div className="w-8 h-8 mb-4 rounded-lg bg-[#FF5500]/10 border border-[#FF5500]/25 flex items-center justify-center">
                            <Icon name="link" className="w-4 h-4 text-[#FF5500]" />
                        </div>
                        <p className="text-xs text-[#FF5500]/90 font-semibold uppercase tracking-wide mb-2">{t('sections.two_ways.faceit.tag')}</p>
                        <h3 id="games-faceit-title" className="text-2xl font-black text-white mb-3">{t('sections.two_ways.faceit.title')}</h3>
                        <p className="text-gray-400 text-sm leading-relaxed mb-4">{t('sections.two_ways.faceit.text')}</p>
                        <ul className="space-y-2 text-sm text-gray-300 mb-6">
                            {(['p1', 'p2', 'p3'] as const).map((k) => (
                                <li key={k} className="flex items-start gap-2">
                                    <Icon name="check" className="w-4 h-4 mt-0.5 shrink-0 text-[#FF5500]" />
                                    {t(`sections.two_ways.faceit.${k}`)}
                                </li>
                            ))}
                        </ul>
                        <div className="mt-auto">
                            <Link to="/lobby" className="inline-flex items-center gap-2 px-6 py-3 rounded-xl border border-[#FF5500]/40 text-[#FF5500] hover:bg-[#FF5500]/10 font-black transition-colors">
                                {t('sections.two_ways.faceit.cta')}
                                <Icon name="chevron-right" className="w-4 h-4" />
                            </Link>
                        </div>
                    </article>
                </div>
                <p className="mt-8 text-center text-sm font-semibold text-gray-400 px-4">
                    <Icon name="lock" className="inline w-4 h-4 mr-1.5 -mt-0.5 text-kaspa-primary" />
                    {t('sections.two_ways.shared')}
                </p>
            </section>

            {/* 2. Schritt-für-Schritt Anleitung CS2 1vs1 */}
            <section id="how" className="py-24 border-t border-kaspa-border relative">
                <div aria-hidden="true" className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[800px] h-[600px] bg-kaspa-primary/5 blur-[150px] rounded-full -z-10" />
                
                <div className="text-center max-w-3xl mx-auto mb-16 px-4">
                    <h2 className="text-3xl md:text-5xl font-black mb-6 uppercase tracking-tight text-white drop-shadow-md">
                        <Trans i18nKey="sections.how_1vs1.title">
                            So funktioniert dein <span className="text-kaspa-primary">KaspaBattle 1vs1</span>
                        </Trans>
                    </h2>
                    <p className="text-lg md:text-xl text-gray-400 font-medium leading-relaxed">
                        {t('sections.how_1vs1.subtitle')}
                    </p>
                </div>

                <div className="max-w-4xl mx-auto flex flex-col gap-6 px-4">
                    {/* Step 1 */}
                    <div className="flex flex-col md:flex-row items-start md:items-center gap-6 p-6 md:p-8 glass-panel rounded-2xl border border-kaspa-primary/15 hover:border-kaspa-primary/35 hover:shadow-glow-subtle focus-within:border-kaspa-primary/35 focus-within:shadow-glow-subtle transition-all duration-300 hover:-translate-y-0.5 relative overflow-hidden group">
                        <div className="absolute top-0 left-0 w-1 h-full bg-kaspa-primary/25 group-hover:bg-kaspa-primary/70 transition-colors" />
                        <div className="w-16 h-16 shrink-0 rounded-2xl bg-kaspa-primary/10 text-kaspa-primary flex items-center justify-center font-black border border-kaspa-primary/30 group-hover:border-kaspa-primary/60 group-hover:bg-kaspa-primary/15 transition-colors duration-300 text-3xl">
                            1
                        </div>
                        <div className="flex-1">
                            <h4 className="text-xl md:text-2xl font-bold text-white mb-2">{t('sections.how_1vs1.steps.1.title')}</h4>
                            <p className="text-gray-400 text-sm md:text-base leading-relaxed mb-4">
                                {t('sections.how_1vs1.steps.1.text')}
                            </p>
                            <div className="flex flex-wrap gap-3">
                                <Link to="/wallet/import" className="inline-flex items-center gap-2 text-sm px-6 py-2.5 bg-kaspa-primary/10 border border-kaspa-primary/30 hover:bg-kaspa-primary/20 text-kaspa-primary font-bold rounded-xl transition-all">
                                    {t('sections.how_1vs1.steps.1.btn_wallet')}
                                </Link>
                                <button onClick={() => isAuthenticated ? startFaceitLink() : startFaceitLogin()} className="inline-flex items-center gap-2 text-sm px-6 py-2.5 bg-[#FF5500]/10 border border-[#FF5500]/30 hover:bg-[#FF5500]/20 text-[#FF5500] font-bold rounded-xl transition-all">
                                    {t('sections.how_1vs1.steps.1.btn_faceit')}
                                </button>
                            </div>
                        </div>
                    </div>

                    {/* Step 2 */}
                    <div className="flex flex-col md:flex-row items-start md:items-center gap-6 p-6 md:p-8 glass-panel rounded-2xl border border-kaspa-primary/15 hover:border-kaspa-primary/35 hover:shadow-glow-subtle focus-within:border-kaspa-primary/35 focus-within:shadow-glow-subtle transition-all duration-300 hover:-translate-y-0.5 relative overflow-hidden group">
                        <div className="absolute top-0 left-0 w-1 h-full bg-kaspa-primary/25 group-hover:bg-kaspa-primary/70 transition-colors" />
                        <div className="w-16 h-16 shrink-0 rounded-2xl bg-kaspa-primary/10 text-kaspa-primary flex items-center justify-center font-black border border-kaspa-primary/30 group-hover:border-kaspa-primary/60 group-hover:bg-kaspa-primary/15 transition-colors duration-300 text-3xl">
                            2
                        </div>
                        <div className="flex-1">
                            <h4 className="text-xl md:text-2xl font-bold text-white mb-2">{t('sections.how_1vs1.steps.2.title')}</h4>
                            <p className="text-gray-400 text-sm md:text-base leading-relaxed">
                                <Trans i18nKey="sections.how_1vs1.steps.2.text">
                                    Gehe in die <Link to="/lobby" className="text-kaspa-primary hover:underline font-bold">Lobby</Link> und nimm eine offene Herausforderung an. 
                                    Alternativ kannst du einfach selbst eine neue Challenge erstellen und deinen gewünschten KAS-Wetteinsatz festlegen.
                                </Trans>
                            </p>
                        </div>
                    </div>

                    {/* Step 3 */}
                    <div className="flex flex-col md:flex-row items-start md:items-center gap-6 p-6 md:p-8 glass-panel rounded-2xl border border-kaspa-primary/15 hover:border-kaspa-primary/35 hover:shadow-glow-subtle focus-within:border-kaspa-primary/35 focus-within:shadow-glow-subtle transition-all duration-300 hover:-translate-y-0.5 relative overflow-hidden group">
                        <div className="absolute top-0 left-0 w-1 h-full bg-kaspa-primary/25 group-hover:bg-kaspa-primary/70 transition-colors" />
                        <div className="w-16 h-16 shrink-0 rounded-2xl bg-kaspa-primary/10 text-kaspa-primary flex items-center justify-center font-black border border-kaspa-primary/30 group-hover:border-kaspa-primary/60 group-hover:bg-kaspa-primary/15 transition-colors duration-300 text-3xl">
                            3
                        </div>
                        <div className="flex-1">
                            <h4 className="text-xl md:text-2xl font-bold text-white mb-2">{t('sections.how_1vs1.steps.3.title')}</h4>
                            <p className="text-gray-400 text-sm md:text-base leading-relaxed">
                                {t('sections.how_1vs1.steps.3.text')}
                            </p>
                        </div>
                    </div>

                    {/* Step 4 */}
                    <div className="flex flex-col md:flex-row items-start md:items-center gap-6 p-6 md:p-8 glass-panel rounded-2xl border border-kaspa-primary/15 hover:border-kaspa-primary/35 hover:shadow-glow-subtle focus-within:border-kaspa-primary/35 focus-within:shadow-glow-subtle transition-all duration-300 hover:-translate-y-0.5 relative overflow-hidden group">
                        <div className="absolute top-0 left-0 w-1 h-full bg-kaspa-primary/25 group-hover:bg-kaspa-primary/70 transition-colors" />
                        <div className="w-16 h-16 shrink-0 rounded-2xl bg-kaspa-primary/10 text-kaspa-primary flex items-center justify-center font-black border border-kaspa-primary/30 group-hover:border-kaspa-primary/60 group-hover:bg-kaspa-primary/15 transition-colors duration-300 text-3xl">
                            4
                        </div>
                        <div className="flex-1">
                            <h4 className="text-xl md:text-2xl font-bold text-white mb-2">{t('sections.how_1vs1.steps.4.title')}</h4>
                            <p className="text-gray-400 text-sm md:text-base leading-relaxed mb-4">
                                <Trans i18nKey="sections.how_1vs1.steps.4.text">
                                    Wechselt jetzt beide zu FACEIT in unseren <strong>KaspaBattle CS2 Club</strong>.
                                    In der Club Queue <em>„1 vs 1"</em> antreten, Match darüber starten und auf dem gleichen Server gegeneinander zocken.
                                </Trans>
                            </p>
                            <div className="flex flex-wrap gap-3">
                                <a href="https://www.faceit.com/en/inv/uanMw8D" target="_blank" rel="noopener noreferrer" className="inline-flex items-center gap-2 text-sm px-6 py-2.5 glass-button font-bold transition-all">
                                    {t('sections.how_1vs1.steps.4.btn_club')}
                                </a>
                                <a href="https://www.faceit.com/en/club/f9352f23-4be0-41a4-a895-9cae45a716c2/queue/3e6b7f39-3c02-40d5-bf5e-17fefc15fdb7/chat" target="_blank" rel="noopener noreferrer" className="inline-flex items-center gap-2 text-sm px-6 py-2.5 bg-kaspa-primary/10 border border-kaspa-primary/40 hover:bg-kaspa-primary/20 text-kaspa-primary font-bold rounded-xl transition-all">
                                    {t('sections.how_1vs1.steps.4.btn_queue')}
                                </a>
                            </div>
                        </div>
                    </div>

                    {/* Step 5 */}
                    <div className="flex flex-col md:flex-row items-start md:items-center gap-6 p-6 md:p-8 glass-panel rounded-2xl border border-kaspa-primary/15 hover:border-kaspa-primary/35 hover:shadow-glow-subtle focus-within:border-kaspa-primary/35 focus-within:shadow-glow-subtle transition-all duration-300 hover:-translate-y-0.5 relative overflow-hidden group">
                        <div className="absolute top-0 left-0 w-1 h-full bg-kaspa-primary/25 group-hover:bg-kaspa-primary/70 transition-colors" />
                        <div className="w-16 h-16 shrink-0 rounded-2xl bg-kaspa-primary/10 text-kaspa-primary flex items-center justify-center font-black border border-kaspa-primary/30 group-hover:border-kaspa-primary/60 group-hover:bg-kaspa-primary/15 transition-colors duration-300 text-3xl">
                            5
                        </div>
                        <div className="flex-1">
                            <h4 className="text-xl md:text-2xl font-bold text-white mb-2">{t('sections.how_1vs1.steps.5.title')}</h4>
                            <p className="text-gray-400 text-sm md:text-base leading-relaxed">
                                {t('sections.how_1vs1.steps.5.text')}
                            </p>
                        </div>
                    </div>

                    {/* Step 6 */}
                    <div className="flex flex-col md:flex-row items-start md:items-center gap-6 p-6 md:p-8 glass-panel rounded-2xl border border-kaspa-primary/15 hover:border-kaspa-primary/35 hover:shadow-glow-subtle focus-within:border-kaspa-primary/35 focus-within:shadow-glow-subtle transition-all duration-300 hover:-translate-y-0.5 relative overflow-hidden group">
                        <div className="absolute top-0 left-0 w-1 h-full bg-kaspa-primary/25 group-hover:bg-kaspa-primary/70 transition-colors" />
                        <div className="w-16 h-16 shrink-0 rounded-2xl bg-kaspa-primary/10 text-kaspa-primary flex items-center justify-center font-black border border-kaspa-primary/30 group-hover:border-kaspa-primary/60 group-hover:bg-kaspa-primary/15 transition-colors duration-300 text-3xl">
                            6
                        </div>
                        <div className="flex-1">
                            <h4 className="text-xl md:text-2xl font-bold text-white mb-2">{t('sections.how_1vs1.steps.6.title')}</h4>
                            <p className="text-gray-400 text-sm md:text-base leading-relaxed">
                                {t('sections.how_1vs1.steps.6.text')}
                            </p>
                        </div>
                    </div>
                </div>
            </section>

            {/* Warum Kaspa? */}
            <section className="py-20 border-t border-kaspa-border relative overflow-hidden">
                {/* Subtle background light island, echoing the hero glow at a much lower key */}
                <div aria-hidden="true" className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[900px] h-[500px] bg-kaspa-primary/5 blur-[140px] rounded-full -z-10" />

                <SectionHeading
                    eyebrow={t('sections.why_kaspa_eyebrow')}
                    title={t('sections.why_kaspa')}
                    subtitle={t('sections.why_kaspa_intro')}
                />

                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
                    {([
                        { iconName: 'bolt'   as const, overline: t('sections.features.finality.overline'), title: t('sections.features.finality.title'),  text: t('sections.features.finality.text') },
                        { iconName: 'blocks' as const, overline: t('sections.features.blocks.overline'),   title: t('sections.features.blocks.title'),    text: t('sections.features.blocks.text') },
                        { iconName: 'coin'   as const, overline: t('sections.features.fees.overline'),     title: t('sections.features.fees.title'),      text: t('sections.features.fees.text') },
                        { iconName: 'shield' as const, overline: t('sections.features.security.overline'), title: t('sections.features.security.title'), text: t('sections.features.security.text') },
                    ]).map((feature) => (
                        <div key={feature.title} className="glass-panel rounded-2xl border border-kaspa-primary/15 hover:border-kaspa-primary/35 hover:shadow-glow-subtle transition-all p-8 text-center hover:-translate-y-1 duration-300">
                            <div className="w-14 h-14 mb-6 mx-auto rounded-xl bg-kaspa-primary/10 border border-kaspa-primary/20 flex items-center justify-center">
                                <Icon name={feature.iconName} className="w-7 h-7 text-kaspa-primary" />
                            </div>
                            <p className="text-xs text-kaspa-primary/80 font-semibold uppercase tracking-wide mb-2">{feature.overline}</p>
                            <h3 className="text-lg font-bold mb-3 text-white">{feature.title}</h3>
                            <p className="text-gray-400 text-sm leading-relaxed">{feature.text}</p>
                        </div>
                    ))}
                </div>
            </section>

            {/* Help build KaspaBattle — Playtest / Contribute / Share feedback */}
            <RecruitmentSection />

            {/* Connect with KaspaBattle */}
            <section className="py-20 border-t border-kaspa-border">
                <SectionHeading title={t('sections.connect.heading')} subtitle={t('sections.connect.intro')} />
                <div className="max-w-2xl mx-auto px-4 flex flex-col items-center gap-8">
                    <SocialLinks iconClassName="w-5 h-5" />
                    <ContactCard className="w-full" />
                </div>
            </section>

            {/* Future / CTA Section */}
            <section className="py-24 border-t border-kaspa-border text-center relative overflow-hidden">
                <div
                    aria-hidden="true"
                    className="pointer-events-none absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[90%] max-w-[560px] h-[320px] md:w-[700px] md:h-[460px] bg-[radial-gradient(ellipse_at_center,rgba(73,234,203,0.16),transparent_70%)] blur-2xl -z-10"
                />
                <h2 className="text-4xl md:text-6xl font-black mb-6 uppercase tracking-tighter">{t('sections.future')}</h2>
                <p className="text-xl text-kaspa-primary mb-4 max-w-2xl mx-auto font-bold tracking-wide">
                    {t('sections.future_subtitle')}
                </p>

                {/* Kaspa Battle Logo */}
                <div className="mb-4 flex justify-center relative">
                    <span className="relative z-10 flex items-center justify-center">
                        <img src="/KaspaBattleLogo.svg" alt="KaspaBattle Logo" className="w-64 h-64 md:w-80 md:h-80 object-contain drop-shadow-[0_0_30px_rgba(112,199,186,0.3)]" />
                    </span>
                </div>

                <h3 className="text-3xl font-black mb-10 text-white">{t('sections.ready')}</h3>

                {/* Stacked CTA Buttons */}
                <div className="flex flex-col items-center justify-center gap-4 max-w-sm mx-auto">
                    {(isAuthenticated && isFullyConnected) ? (
                        <>
                            <Link to="/lobby" className="btn-primary w-full px-12 py-5 text-xl shadow-glow-primary hover:shadow-glow-primary-lg transition-shadow text-kaspa-dark font-black flex items-center justify-center">
                                {t('navigation.back_to_lobby')}
                            </Link>
                            {testMode && (
                                <Link to="/tournaments" className="w-full px-12 py-4 text-lg border-2 border-kaspa-primary/50 hover:border-kaspa-primary text-kaspa-primary font-black rounded-xl flex items-center justify-center gap-2 transition-all hover:bg-kaspa-primary/5">
                                    Browse Tournaments
                                </Link>
                            )}
                        </>
                    ) : (
                        <>
                            <Link to="/wallet/import" className="btn-primary w-full px-10 py-5 text-lg flex items-center gap-3 text-kaspa-dark font-black justify-center">
                                {t('wallet.connect')}
                            </Link>
                            <button onClick={() => (isAuthenticated ? startFaceitLink() : startFaceitLogin())} className="w-full bg-orange-600 hover:bg-orange-500 px-10 py-5 text-lg flex items-center gap-3 text-white font-black rounded-xl justify-center transition-all">
                                {t('navigation.faceit_login')}
                            </button>
                            {testMode && (
                                <Link to="/tournaments" className="w-full px-12 py-4 text-base border border-kaspa-primary/30 hover:border-kaspa-primary/60 text-kaspa-primary font-semibold rounded-xl flex items-center justify-center gap-2 transition-all hover:bg-kaspa-primary/5">
                                    View Tournaments
                                </Link>
                            )}
                        </>
                    )}
                </div>
            </section>

            {/* Tournament Mode Callout — testMode only */}
            {testMode && (
            <section className="py-16 border-t border-kaspa-border">
                <div className="max-w-4xl mx-auto px-4">
                    <div className="glass-panel rounded-2xl relative overflow-hidden border border-kaspa-primary/20 p-8 md:p-12">
                        <div aria-hidden="true" className="absolute top-0 right-0 w-64 h-64 bg-kaspa-primary/5 blur-3xl rounded-full -z-0" />
                        <div className="relative z-10">
                            <div className="inline-flex items-center gap-2 px-3 py-1 bg-kaspa-primary/10 border border-kaspa-primary/20 rounded-full text-kaspa-primary text-xs font-bold tracking-widest uppercase mb-6">
                                {t('sections.tournament_callout.badge')}
                            </div>
                            <h2 className="text-3xl md:text-4xl font-black text-white mb-4 leading-tight">
                                {t('sections.tournament_callout.title_prefix')}{' '}
                                <span className="text-kaspa-primary">{t('sections.tournament_callout.title_highlight')}</span>
                            </h2>
                            <p className="text-gray-400 text-lg mb-8 max-w-xl">
                                {t('sections.tournament_callout.text')}
                            </p>
                            <div className="grid sm:grid-cols-3 gap-4 mb-8">
                                {[
                                    { iconName: 'list' as const, title: t('sections.tournament_callout.feature_bracket_title'), text: t('sections.tournament_callout.feature_bracket_text') },
                                    { iconName: 'coin' as const, title: t('sections.tournament_callout.feature_payout_title'), text: t('sections.tournament_callout.feature_payout_text') },
                                    { iconName: 'dispute' as const, title: t('sections.tournament_callout.feature_dispute_title'), text: t('sections.tournament_callout.feature_dispute_text') },
                                ].map((f, i) => (
                                    <div key={i} className="bg-black/20 border border-white/5 rounded-xl p-4">
                                        <Icon name={f.iconName} className="w-5 h-5 text-kaspa-primary mb-2" />
                                        <div className="font-bold text-white text-sm mb-1">{f.title}</div>
                                        <div className="text-gray-500 text-xs">{f.text}</div>
                                    </div>
                                ))}
                            </div>
                            <Link
                                to="/tournaments"
                                id="landing-tournaments-cta"
                                className="btn-primary inline-flex items-center gap-2 px-8 py-4 text-kaspa-dark font-black"
                            >
                                {t('sections.tournament_callout.cta')}
                                <Icon name="chevron-right" className="w-4 h-4" />
                            </Link>
                        </div>
                    </div>
                </div>
            </section>
            )}
        </div>
    );
}
