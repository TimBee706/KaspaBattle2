import { useTranslation } from 'react-i18next';

export function WhitepaperPage() {
  const { t, i18n } = useTranslation();

  const isGerman = i18n.language.startsWith('de');

  return (
    <div className="max-w-3xl mx-auto py-16 px-4 text-white">
      <h1 className="text-3xl font-bold mb-4">
        {t('footer_links.whitepaper')}
      </h1>

      {isGerman ? (
        <>
          <p className="text-gray-300 mb-4">
            Das KaspaBattle Whitepaper beschreibt, wie die Plattform Kaspa als schnelle, günstige und
            vollständig dezentrale Settlement-Schicht für Esports-Wetten nutzt. Es erklärt das
            Multisig-Escrow-Modell, die Orakel-Architektur sowie das Sicherheits- und
            Gebührenmodell der Plattform.
          </p>
          <p className="text-gray-400 mb-4 text-sm">
            In der aktuellen Beta-Phase arbeiten wir noch an der finalen Version des Whitepapers.
            Sobald das vollständige PDF vorliegt, wird es hier direkt eingebunden und als Download
            zur Verfügung stehen.
          </p>
          <p className="text-gray-400 text-sm">
            Für technische Details zu Kaspa selbst empfehlen wir zusätzlich einen Blick in die
            offiziellen Kaspa-Dokumentationen und das Kaspa-Whitepaper der Core-Entwickler.
          </p>
        </>
      ) : (
        <>
          <p className="text-gray-300 mb-4">
            The KaspaBattle whitepaper explains how the platform uses Kaspa as a fast, low-fee and
            fully decentralized settlement layer for esports wagering. It describes the multisig
            escrow model, the oracle architecture and the platform&apos;s security and fee design.
          </p>
          <p className="text-gray-400 mb-4 text-sm">
            During the current beta phase we are still finalizing the full whitepaper. As soon as the
            complete PDF is available, it will be embedded on this page and offered for download.
          </p>
          <p className="text-gray-400 text-sm">
            For additional technical context about Kaspa itself we also recommend checking the
            official Kaspa documentation and core whitepaper from the main Kaspa developers.
          </p>
        </>
      )}
    </div>
  );
}
