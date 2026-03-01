import { useTranslation } from 'react-i18next';

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
                        <a href="#" className="hover:text-kaspa-primary transition-colors">Whitepaper</a>
                        <a href="#" className="hover:text-kaspa-primary transition-colors">AGB</a>
                        <a href="#" className="hover:text-kaspa-primary transition-colors">Discord</a>
                        <a href="#" className="hover:text-kaspa-primary transition-colors">Support</a>
                    </div>
                </div>
            </div>
        </footer>
    );
}
