import React from 'react';
import { useLanguage } from '../hooks/useLanguage';

export const LanguageToggle: React.FC = () => {
    const { currentLanguage, setLanguage, languages } = useLanguage();

    return (
        <div
            className="fixed top-4 right-4 z-[1000] flex bg-kaspa-card/80 backdrop-blur-md border border-kaspa-border rounded-lg p-1 shadow-xl"
            aria-label="Switch language"
        >
            {languages.map((lang) => (
                <button
                    key={lang.code}
                    onClick={() => setLanguage(lang.code)}
                    aria-pressed={currentLanguage === lang.code}
                    className={`
            px-3 py-1.5 text-xs font-black rounded-md transition-all
            ${currentLanguage === lang.code
                            ? 'bg-kaspa-primary text-kaspa-dark shadow-lg'
                            : 'text-gray-400 hover:text-white hover:bg-kaspa-dark/50'
                        }
          `}
                >
                    {lang.label}
                </button>
            ))}
        </div>
    );
};
