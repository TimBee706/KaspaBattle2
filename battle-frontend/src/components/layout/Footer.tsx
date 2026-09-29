import { useTranslation } from 'react-i18next';
import { Link } from 'react-router-dom';
import { SocialLinks } from '../common/SocialLinks';
import { Icon } from '../Icon';
import { publicLinks } from '../../config/publicLinks';
import { PUBLIC_CONTACT_EMAIL } from '../../config/constants';

export function Footer() {
    const { t } = useTranslation();
    const hasEmail = Boolean(PUBLIC_CONTACT_EMAIL);

    return (
        <footer className="bg-kaspa-dark border-t border-kaspa-border mt-auto">
            <div className="container mx-auto px-4 py-12">
                <div className="grid grid-cols-1 md:grid-cols-[1.4fr_1fr_1.2fr] gap-10 mb-10">
                    {/* Brand + description */}
                    <div>
                        <Link to="/" className="inline-flex items-center gap-2 text-lg font-bold text-kaspa-primary tracking-tighter mb-3">
                            <img src="/KaspaBattleLogo.svg" alt="KaspaBattle Logo" className="h-7 w-auto" />
                            KASPABATTLE
                        </Link>
                        <p className="text-sm text-gray-400 max-w-sm leading-relaxed mb-3">{t('footer.description')}</p>
                        <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-kaspa-primary/10 border border-kaspa-primary/20 text-kaspa-primary text-[10px] font-black uppercase tracking-wide">
                            {t('footer.beta_status')}
                        </span>
                    </div>

                    {/* Project links */}
                    <div>
                        <h3 className="text-xs font-bold uppercase tracking-widest text-gray-500 mb-4">
                            {t('footer.nav_heading')}
                        </h3>
                        <ul className="flex flex-col gap-2.5 text-sm">
                            <li>
                                <a href={publicLinks.githubProfile} target="_blank" rel="noopener noreferrer" className="text-gray-400 hover:text-kaspa-primary transition-colors">
                                    {t('footer_links.github_profile')}
                                </a>
                            </li>
                            <li>
                                <a href={publicLinks.githubRepository} target="_blank" rel="noopener noreferrer" className="text-gray-400 hover:text-kaspa-primary transition-colors">
                                    {t('footer_links.github_repo')}
                                </a>
                            </li>
                            <li>
                                <a href={publicLinks.githubIssues} target="_blank" rel="noopener noreferrer" className="text-gray-400 hover:text-kaspa-primary transition-colors">
                                    {t('footer_links.github_issues')}
                                </a>
                            </li>
                            <li>
                                <Link to="/whitepaper" className="text-gray-400 hover:text-kaspa-primary transition-colors">
                                    {t('footer_links.whitepaper')}
                                </Link>
                            </li>
                            <li>
                                <Link to="/support" className="text-gray-400 hover:text-kaspa-primary transition-colors">
                                    {t('footer_links.support')}
                                </Link>
                            </li>
                            <li>
                                <Link to="/terms" className="text-gray-400 hover:text-kaspa-primary transition-colors">
                                    {t('footer_links.terms')}
                                </Link>
                            </li>
                        </ul>
                    </div>

                    {/* Social + contact */}
                    <div>
                        <h3 className="text-xs font-bold uppercase tracking-widest text-gray-500 mb-4">
                            {t('footer.social_heading')}
                        </h3>
                        <SocialLinks className="mb-6" />
                        <h3 className="text-xs font-bold uppercase tracking-widest text-gray-500 mb-3">
                            {t('footer.contact_heading')}
                        </h3>
                        {hasEmail ? (
                            <a
                                href={`mailto:${PUBLIC_CONTACT_EMAIL}`}
                                className="text-sm text-gray-400 hover:text-kaspa-primary transition-colors inline-flex items-center gap-2"
                            >
                                <Icon name="mail" className="w-4 h-4 shrink-0" />
                                <span className="break-all">{PUBLIC_CONTACT_EMAIL}</span>
                            </a>
                        ) : (
                            <a
                                href={publicLinks.githubIssueNew}
                                target="_blank"
                                rel="noopener noreferrer"
                                className="text-sm text-gray-400 hover:text-kaspa-primary transition-colors inline-flex items-center gap-2"
                            >
                                <Icon name="github" className="w-4 h-4 shrink-0" />
                                {t('sections.recruit.feedback.cta_issue')}
                            </a>
                        )}
                    </div>
                </div>

                <div id="testnet-details" className="scroll-mt-24 border-t border-kaspa-border/60 pt-6 flex flex-col md:flex-row justify-between items-start md:items-center gap-3">
                    <p className="text-gray-500 text-xs max-w-2xl leading-relaxed">{t('footer.disclaimer')}</p>
                    <p className="text-gray-500 text-xs shrink-0">{t('footer.copyright')}</p>
                </div>
            </div>
        </footer>
    );
}
