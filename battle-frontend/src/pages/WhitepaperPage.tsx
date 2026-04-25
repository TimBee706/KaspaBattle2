import { useTranslation } from 'react-i18next';

export function WhitepaperPage() {
  const { t } = useTranslation();

  return (
    <div className="max-w-3xl mx-auto py-16 px-4 text-white">
      <h1 className="text-3xl font-bold mb-4">
        {t('footer_links.whitepaper')}
      </h1>
      <p className="text-gray-400 text-sm">
        KaspaBattle Whitepaper page placeholder. Replace this content with the actual whitepaper content or embed once available.
      </p>
    </div>
  );
}
