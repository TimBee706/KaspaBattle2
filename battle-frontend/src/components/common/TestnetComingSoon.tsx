import { useTranslation } from 'react-i18next';
import { Icon } from '../Icon';
import { NODE_HELP_CONTACT_EMAIL } from '../../config/constants';

/**
 * Uniform, non-blocking status for everything that needs the Kaspa testnet (wallet, FACEIT
 * settlement, deposits, escrow). It is information only – it never disables Free Play.
 */
export function TestnetComingSoon({ showHelp = true, className = '' }: { showHelp?: boolean; className?: string }) {
    const { t } = useTranslation();
    return (
        <div
            role="note"
            data-testid="testnet-coming-soon"
            className={`flex items-start gap-3 rounded-xl border border-amber-500/30 bg-amber-900/10 px-4 py-3 ${className}`}
        >
            <Icon name="clock" className="mt-0.5 h-4 w-4 shrink-0 text-amber-400" />
            <div className="min-w-0 text-sm">
                <p className="font-bold text-amber-300">{t('freeplay.testnet.status')}</p>
                {showHelp && (
                    <p className="mt-0.5 text-xs text-gray-400">
                        {t('freeplay.testnet.help_short')}{' '}
                        <a href={`mailto:${NODE_HELP_CONTACT_EMAIL}`} className="font-semibold text-kaspa-primary underline-offset-2 hover:underline">
                            {NODE_HELP_CONTACT_EMAIL}
                        </a>
                    </p>
                )}
            </div>
        </div>
    );
}
