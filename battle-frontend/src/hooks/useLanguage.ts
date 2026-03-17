import { useTranslation } from 'react-i18next';

export type Language = 'de' | 'en';

export const LANGUAGES: { code: Language; label: string }[] = [
    { code: 'de', label: 'DE' },
    { code: 'en', label: 'EN' },
];

export function useLanguage() {
    const { i18n } = useTranslation();

    const currentLanguage = i18n.language as Language;

    const setLanguage = (lang: Language) => {
        i18n.changeLanguage(lang);
        localStorage.setItem('preferred_language', lang);
    };

    return {
        currentLanguage,
        setLanguage,
        languages: LANGUAGES,
    };
}
