import { useTranslation } from 'react-i18next';

export function TermsPage() {
  const { t, i18n } = useTranslation();

  const isGerman = i18n.language.startsWith('de');

  return (
    <div className="max-w-3xl mx-auto py-16 px-4 text-white">
      <h1 className="text-3xl font-bold mb-4">
        {t('footer_links.terms')}
      </h1>

      {isGerman ? (
        <>
          <p className="text-gray-300 mb-4">
            Diese Seite wird zukünftig die vollständigen Allgemeinen Geschäftsbedingungen (AGB) von
            KaspaBattle enthalten. Sie regeln unter anderem die Nutzung der Plattform, das
            Einzahlen von Einsätzen, die Abwicklung von Matches sowie Haftungs- und
            Risikohinweise.
          </p>
          <p className="text-gray-400 mb-4 text-sm">
            Aktuell befindet sich der rechtliche Text noch in Ausarbeitung. Bis zur finalen
            Veröffentlichung gelten die im Onboarding und in der App dargestellten Hinweise zu
            Beta-Status, Testnet-Einsatz und Risikohinweisen.
          </p>
          <p className="text-gray-400 text-sm">
            Bitte nutze KaspaBattle nur, wenn du die Funktionsweise von Kryptowährungen,
            insbesondere von Kaspa, verstehst und dir der damit verbundenen Risiken bewusst bist.
          </p>
        </>
      ) : (
        <>
          <p className="text-gray-300 mb-4">
            This page will host the full KaspaBattle Terms &amp; Conditions. They will cover platform
            usage, deposits and withdrawals, match handling as well as liability and risk
            disclosures.
          </p>
          <p className="text-gray-400 mb-4 text-sm">
            The final legal text is still being prepared during the beta phase. Until it is
            published, please refer to the notices shown during onboarding and inside the app
            regarding beta status, testnet usage and risk information.
          </p>
          <p className="text-gray-400 text-sm">
            Only use KaspaBattle if you understand how cryptocurrencies – especially Kaspa – work and
            you are aware of the associated risks.
          </p>
        </>
      )}
    </div>
  );
}
