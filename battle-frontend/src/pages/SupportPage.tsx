import { useTranslation } from 'react-i18next';

export function SupportPage() {
  const { t, i18n } = useTranslation();

  const isGerman = i18n.language.startsWith('de');

  return (
    <div className="max-w-3xl mx-auto py-16 px-4">
      <div className="mb-10">
        <h1 className="text-4xl font-black text-white uppercase tracking-tighter mb-2">
          {t('footer_links.support')}
        </h1>
      </div>
      <div className="glass-panel p-8 leading-relaxed">

      {isGerman ? (
        <>
          <p className="text-gray-300 mb-4">
            Du hast Fragen zu KaspaBattle, Feedback oder ein Problem mit einem Match oder deiner
            Wallet-Integration? Unser kleines Team hilft dir gerne weiter.
          </p>
          <p className="text-gray-300 mb-4">
            Der schnellste Weg zum Support ist eine E-Mail an:
          </p>
          <p className="mb-6">
            <a
              href="mailto:info@kaspabattle.com"
              className="text-kaspa-primary hover:underline break-all"
            >
              info@kaspabattle.com
            </a>
          </p>
          <p className="text-gray-400 text-sm mb-2 font-semibold uppercase tracking-wide">
            Damit wir dir schneller helfen können, füge bitte folgende Infos hinzu:
          </p>
          <ul className="list-disc list-inside text-gray-400 text-sm space-y-1 mb-6">
            <li>Dein FACEIT-Nickname (falls vorhanden)</li>
            <li>Die Match-ID oder Lobby-ID, falls es um ein konkretes Match geht</li>
            <li>Deine Kaspa-Adresse, sofern ein On-Chain-Problem vorliegt</li>
            <li>Einen kurzen Beschreibungstext, was genau passiert ist</li>
          </ul>
          <p className="text-gray-500 text-xs">
            Hinweis: KaspaBattle befindet sich noch in der Beta-Phase. Kleinere Bugs und visuelle
            Glitches können auftreten – dein Feedback hilft uns, die Plattform schneller zu
            verbessern.
          </p>
        </>
      ) : (
        <>
          <p className="text-gray-300 mb-4">
            Questions about KaspaBattle, feedback or an issue with a match or your wallet
            integration? Our small team is happy to help.
          </p>
          <p className="text-gray-300 mb-4">
            The fastest way to reach support is via email:
          </p>
          <p className="mb-6">
            <a
              href="mailto:info@kaspabattle.com"
              className="text-kaspa-primary hover:underline break-all"
            >
              info@kaspabattle.com
            </a>
          </p>
          <p className="text-gray-400 text-sm mb-2 font-semibold uppercase tracking-wide">
            To help us resolve your issue quickly, please include:
          </p>
          <ul className="list-disc list-inside text-gray-400 text-sm space-y-1 mb-6">
            <li>Your FACEIT nickname (if applicable)</li>
            <li>The match ID or lobby ID if it relates to a specific match</li>
            <li>Your Kaspa address if there is an on-chain issue</li>
            <li>A short description of what happened and what you expected</li>
          </ul>
          <p className="text-gray-500 text-xs">
            Note: KaspaBattle is still in beta. Minor bugs and visual glitches may occur – your
            feedback helps us improve the platform faster.
          </p>
        </>
      )}
      </div>
    </div>
  );
}
