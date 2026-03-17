import React from 'react';
import { useLanguage } from '../hooks/useLanguage';
import { useTranslation } from 'react-i18next';

export const LanguageToggle: React.FC = () => {
    const { currentLanguage, setLanguage, languages } = useLanguage();
    const { t } = useTranslation();

    return (
        <select
            value={currentLanguage}
            onChange={(e) => setLanguage(e.target.value as any)}
            className="bg-transparent text-gray-300 font-bold focus:outline-none cursor-pointer border-none hover:text-white transition-colors uppercase text-sm"
            aria-label={t('common.header.language', 'Language')}
        >
            {languages.map((lang) => (
                <option key={lang.code} value={lang.code} className="bg-kaspa-dark text-white">
                    {lang.label}
                </option>
            ))}
        </select>
    );
};
