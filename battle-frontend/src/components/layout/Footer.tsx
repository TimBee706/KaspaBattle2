import { useTranslation } from 'react-i18next';
import { Link } from 'react-router-dom';

export function Footer() {
    const { t } = useTranslation();

    return (
        <footer className="bg-kaspa-dark border-t border-kaspa-border py-8 mt-auto">
            <div className="container mx-auto px-4">
                <div className="flex flex-col md:flex-row justify-between items-center gap-4">
                    <div className="text-gray-500 text-sm">
                        {t('footer.copyright')}
                    </div>

                    <div className="flex items-center gap-6 text-sm text-gray-400">
                        <Link to="/whitepaper" className="hover:text-kaspa-primary transition-colors">
                            {t('footer_links.whitepaper')}
                        </Link>
                        <Link to="/terms" className="hover:text-kaspa-primary transition-colors">
                            {t('footer_links.terms')}
                        </Link>
                        <a
                            href="https://x.com/KaspaBattle"
                            target="_blank"
                            rel="noopener noreferrer"
                            className="hover:text-kaspa-primary transition-colors"
                        >
                            {t('footer_links.discord')}
                        </a>
                        <Link
                            to="/support"
                            className="hover:text-kaspa-primary transition-colors"
                        >
                            {t('footer_links.support')}
                        </Link>
                    </div>
                </div>
            </div>
        </footer>
    );
}
