# Kaspa Battle 2.0 – Turnierplattform auf Basis von FaceIT und Kaspa Covenants

## Abstract

Dieses Whitepaper skizziert eine mögliche **Version 2.0 von Kaspa Battle** – eine wettbewerbsorientierte Turnierplattform, die **FaceIT** als Matchmaking- und Anti-Cheat-Infrastruktur nutzt und gleichzeitig **Kaspa Covenants (Toccata / Testnet 12)** als Basis für transparente, non‑custodiale Preisgelder einsetzt.[web:61][web:63][web:74][web:78] 

Am Beispiel eines **5‑Team‑Turniers mit jeweils 5 Spielern (5v5)** wird ein konkreter Ablaufplan beschrieben, der es Entwicklern erleichtert, die technische Architektur und die notwendigen Komponenten (Backend, Smart‑UTXO‑Flows, FaceIT‑Integration, Wallet‑Handling) zu implementieren.[web:72][web:74][web:76][web:78]

---

## 1. Zielbild von Kaspa Battle 2.0

Kaspa Battle 2.0 verfolgt drei Kernziele:

1. **Trust‑minimierte Preisgelder**  
   Preisgelder und Buy‑ins werden auf **Kaspa** über Covenants gehalten, sodass weder Plattformbetreiber noch einzelne Admins allein über die Gelder verfügen können.[web:60][web:61][web:63]

2. **Nahtlose Integration mit FaceIT**  
   FaceIT bleibt die Quelle für Match‑Erstellung, Lobby‑Handling, Anti‑Cheat und Ergebnisdaten; Kaspa Battle ergänzt lediglich **Finanz‑Logik und Turnierökonomie**.[web:72][web:74][web:78]

3. **Erweiterbare Turnierlogik**  
   Vom einfachen 1v1 bis zu komplexeren Team‑Turnieren mit mehreren Preisrängen (z.B. Top‑3‑Payout) sollen alle Varianten auf einem gemeinsamen State‑Machine‑Modell aufbauen.[web:60][web:76]

---

## 2. Technische Grundlagen

### 2.1 FaceIT als Turnier-Backbone

FaceIT stellt folgende zentrale Funktionen bereit:

- **Team- und Spieler-Verwaltung** (Teams, Lineups, Spielerprofile).[web:72][web:74]
- **Turnier-Objekte** (Championships/Tournaments) inkl. Struktur, Slots, Format (Single-/Double-Elimination, Swiss, etc.).[web:74][web:75][web:76]
- **Matchrooms & Match-Konfiguration** (Server, Maps, Rundenkonfiguration, Spectator Rules).[web:74][web:76]
- **Turnier-Lifecycle**: Sign‑up, Check‑in, Rundenplanung, Matchstart, Ergebnis-Erfassung.[web:74][web:76]
- **Daten-API** (REST) für Turnier-, Team- und Matchdaten.[web:78]

Kaspa Battle 2.0 baut darauf auf, indem es FaceIT‑Turniere spiegelt, zusätzliche ökonomische Regeln hinzufügt und das Ergebnis-Event aus FaceIT als Trigger für On‑Chain‑Payouts verwendet.[web:74][web:76][web:78]

### 2.2 Kaspa Covenants und Testnet 12

Kaspa Covenants (Covenants++) ermöglichen **programmierbare Ausgaberegeln auf UTXO‑Basis**, ohne eine globale EVM‑VM einzuführen.[web:60][web:61][web:65]

Wichtige Eigenschaften:

- Eine UTXO kann Regeln enthalten, **wie** und **an wen** sie in Zukunft ausgegeben werden darf (z.B. nur an eine feste Adressliste, nur nach einem bestimmten Block, nur wenn bestimmte Output‑Struktur erfüllt ist).[web:60][web:65]
- **Testnet 12** ist das erste öffentliche Netzwerk mit aktivierten Covenants (KIP‑17), auf dem Entwickler Escrow‑, Vault‑ und zeitverzögerte Spending-Logik testen können.[web:61][web:63][web:67]
- Covenants++ fassen die notwendigen **Opcodes und Covenant IDs** zusammen, mit denen sich ganze UTXO‑Familien (z.B. alle UTXOs eines Turniers) logisch gruppieren lassen.[web:60]

Für Kaspa Battle 2.0 bedeutet das: Turnier-Pots können als **Covenant‑gebundene UTXOs** modelliert werden, die nur über genau definierte Payout‑Transaktionen an mehrere Gewinner und Fees ausgegeben werden dürfen.[web:60][web:61]

---

## 3. Systemarchitektur von Kaspa Battle 2.0

### 3.1 Komponentenübersicht

Kaspa Battle 2.0 besteht aus folgenden Hauptkomponenten:

1. **Web-/Desktop-Client (Kaspa Battle UI)**  
   - Authentifizierung via FaceIT OAuth und Kaspa-Wallet (z.B. Browser-Erweiterung oder Desktop-Client).
   - Anzeige von Turnieren, Pot-Größen, Buy‑ins, Ladder/Bracket.
   - Signieren von Einzahlungs- und Payout-Transaktionen.

2. **Kaspa Battle Backend**  
   - REST-/WebSocket-API für Frontend.
   - Verwaltung von Turnieren, Spielern, Teams (Spiegelung von FaceIT IDs).
   - Orchestrierung der On‑Chain‑Flows (Covenant‑Generierung, Broadcasts, Statusverfolgung).
   - Persistenz (Datenbank) für Turnier-Metadaten, Wallet‑Bindings, Covenant IDs, UTXO‑Referenzen.

3. **FaceIT Integration Service**  
   - Nutzung der FaceIT Data API (Championships, Matches, Teams).[web:74][web:76][web:78]
   - Webhook-/Polling-Mechanismus für Match-Ergebnisse und Turnierstatus.
   - Mapping FaceIT ↔ Kaspa Battle (z.B. FaceIT-Thournament-ID → Kaspa‑Tournament‑ID).

4. **Kaspa Node + Covenant Service**  
   - Voller Kaspa Node (Mainnet/Testnet 12) für Broadcast, UTXO‑Suche, Fee-Berechnung.
   - Covenant‑Script‑Vorlagen & Builder zur dynamischen Erzeugung von Turnier‑Covenants.[web:60][web:61]

5. **Monitoring & Admin Panel**  
   - Turnierüberwachung, Fehlerhandling (z.B. nicht gespielte Matches, strittige Ergebnisse).
   - Manuelle Intervention in Ausnahmefällen (z.B. Disconnect zwischen FaceIT und Chain, aber transparent und auditierbar).

### 3.2 Datenmodell (vereinfacht)

Wichtige Entitäten:

- `User`: Verknüpfung von FaceIT-User-ID, Kaspa-Adresse(n), internen Rollen.
- `Team`: FaceIT-Team-ID, Name, Mitgliederliste, Link zu Kaspa‑Battle-Team.
- `Tournament`: Metadaten (Name, Spiel, Region, Format, Buy‑in, Preispool-Regeln).[web:74][web:76]
- `TournamentPot`: Referenz auf Covenant‑ID + Liste der zugehörigen UTXOs (Eingänge & spätere Payouts).
- `Match`: FaceIT-Match-ID, Teams, Status, Ergebnis, verlinkte On‑Chain‑Aktionen (z.B. Bonusprämien pro Sieg).
- `PayoutRule`: z.B. Top‑3‑Verteilung (z.B. 60/25/15) + Plattform‑Fee (z.B. 5 % vom Gesamtpot).

---

## 4. Beispielturnier: 5 Teams à 5 Spieler

### 4.1 Turnierparameter

- **Spiel**: CS2 5v5.
- **Teams**: 5 Teams x 5 Spieler (Team A–E).
- **Format**: Single-Elimination mit 5 Teams (eine Mannschaft erhält ggf. ein Freilos in Runde 1, oder es werden Play‑In‑Matches gespielt).[web:76]
- **Buy‑in pro Team**: z.B. 100 KAS.
- **Gesamtpot**: 500 KAS (ohne zusätzliche Sponsor-Beiträge).
- **Payout-Regel**: Top 3 – Platz 1: 60 %, Platz 2: 25 %, Platz 3: 15 %.
- **Plattform-Fee**: 5 % vom Gesamtpot, vor der Verteilung abgezogen.

### 4.2 Tournament Lifecycle (FaceIT‑Sicht)

1. **Turnier-Erstellung (Organizer / Kaspa Battle)**  
   - Organizer legt das Turnier auf FaceIT an: Game = CS2, Mode = 5v5, Format = Single-Elimination, Slots = 5 Teams.[web:74][web:76]
   - Turnierdetails (Beschreibung, Regeln, Preise) werden in FaceIT gepflegt; Kaspa Battle verlinkt den On‑Chain‑Pot in der Beschreibung.

2. **Team-Erstellung & Join**  
   - Teams werden auf FaceIT erstellt oder bestehen bereits; Spieler joinen Teams über FaceIT UI.[web:72][web:74]
   - Teams melden sich für das Turnier an (Join mit bestehendem Team).[web:72][web:74]

3. **Check‑in & Seed**  
   - Kurz vor Start führen Teams einen Check‑in durch; No‑Shows werden entfernt.[web:76]
   - Organizer (oder automatisiert) vergibt Seeds oder verwendet FaceIT Skill‑Level für Seeding.[web:75]

4. **Bracket-Generierung & Match-Start**  
   - FaceIT generiert das Bracket und erstellt Matchrooms entsprechend dem Format.[web:76]
   - Spieler joinen Matchrooms, spielen Matches; Ergebnisse werden in FaceIT erfasst (automatisch via Game Integration oder manuell bestätigt).[web:74][web:76]

5. **Turnierabschluss & Bestimmung der Plätze**  
   - Nach dem Finale stehen Platz 1 und 2 fest; ein optionaler 3rd Place Decider kann Platz 3 bestimmen.[web:76]
   - FaceIT hält finalen Stand des Brackets und die Platzierungsliste.

Kaspa Battle 2.0 liest diesen Lifecycle über die Daten-API und Webhooks und koppelt daran die On‑Chain‑Schritte.

---

## 5. On-Chain-Design für Turnier-Pots

### 5.1 Zustände eines TournamentPot

Wir modellieren den TournamentPot als einfache State-Machine auf UTXO-Ebene:

1. **Created** – Turnier existiert, aber keine Einzahlungen.
2. **Funding** – Teams zahlen ihren Buy‑in ein.
3. **Locked** – Buy‑in-Fenster geschlossen, Pot fixiert.
4. **InProgress** – Matches laufen; Pot bleibt unverändert.
5. **ReadyForPayout** – FaceIT-Ergebnisse final; Payout-Pfad kann aktiviert werden.
6. **PaidOut** – Pot auf Gewinner + Fee‑Wallet verteilt.
7. **Refunded** (optional) – Fallback, falls Turnier abgebrochen wird.

Jeder Zustand entspricht einem bestimmten Muster an UTXOs und erlaubten Nachfolgetransaktionen, die über Covenants++ definiert sind.[web:60][web:65]

### 5.2 Funding-Flow (Teams zahlen ein)

**Akteure:**

- Team-Captain (oder definierter Finance-Manager des Teams).
- Kaspa Battle Backend.
- Kaspa Node / Covenant Service.

**Schritte:**

1. **Turnier anzeigen**  
   - Client ruft `/tournaments/{id}` beim Kaspa Battle Backend auf.
   - Backend gibt Metadaten zurück (Buy‑in, Preispool-Regel, FaceIT-Link, Deadline, Kaspa‑Zieladresse/Covenant‑Info).

2. **Funding-Transaktion vorbereiten**  
   - Client generiert über Wallet eine Transaktion `TxTeamBuyIn`, die 100 KAS von der Team-Kasse (oder Captains-Wallet) an die `TournamentPot`-Adresse sendet.
   - Script enthält eine `covenant_binding`, die die UTXO dem Covenant des Turniers zuordnet.[web:60]

3. **Broadcast & Bestätigung**  
   - Transaktion wird an den Kaspa Node gesendet.
   - Backend überwacht den Chain-Mempool und bestätigt, sobald die UTXO im Covenant-Set des Turniers erscheint.

4. **State-Update**  
   - Sobald alle 5 Teams bezahlt haben, wechselt der TournamentPot von `Funding` zu `Locked`.
   - Zusätzliche Einzahlungen werden per Covenant-Regel abgewiesen (Output-Pattern stimmt nicht mit erlaubtem Funding-Pattern überein).[web:60][web:65]

### 5.3 Payout-Design (Top 3 + Fee)

Wir definieren eine Payout-Regel:

- Gesamtsumme: 5 x 100 KAS = 500 KAS.
- Fee: 5 % = 25 KAS.
- Verbleibender Pot: 475 KAS.
- Platz 1: 60 % von 475 KAS ≈ 285 KAS.
- Platz 2: 25 % von 475 KAS ≈ 118,75 KAS.
- Platz 3: 15 % von 475 KAS ≈ 71,25 KAS.

In der Praxis kann man mit integerbasierten Einheiten (z.B. Satoshis) exakte Aufteilungen definieren und verbleibende Einheiten minimal an Platz 1 zuschlagen.

**Payout-Transaktion `TxTournamentPayout`:**

- Inputs: alle Funding-UTXOs (oder eine aggregierte Pot-UTXO).
- Outputs:
  - `OutFee` → Plattform-Fee-Adresse (25 KAS).
  - `OutP1` → Team 1 Wallet (Platz 1).
  - `OutP2` → Team 2 Wallet (Platz 2).
  - `OutP3` → Team 3 Wallet (Platz 3).

Covenant-Regeln stellen sicher:

- Gesamt-Output-Summe = Gesamt-Pot (minus Miner Fees).
- Genau vier Outputs in der oben definierten Struktur.
- Adressen entsprechen den zum Zeitpunkt des Turnierstarts registrierten Team-Wallets.
- Transaktion wird nur nach Eintreten des Turnier-Endzustands akzeptiert (z.B. via Blockheight/Time‑lock oder Signatur eines autorisierten Resolver‑Inputs).[web:60][web:61][web:65]

### 5.4 Refund-Flow (Fallback)

Falls das Turnier nicht regulär abgeschlossen wird (z.B. technische Probleme bei FaceIT oder massiver Ausfall), soll ein Refund möglich sein:

- `TxRefund` verteilt den Pot zurück auf die ursprünglichen Funding-Adressen der Teams.
- Covenants definieren einen **alternativen erlaubten Pfad**, der erst nach einer bestimmten Zeitspanne (z.B. X Blöcke nach geplantem Enddatum) aktiviert werden darf, sofern kein Payout erfolgt ist.
- Dies verhindert, dass Gelder dauerhaft in einem defekten Turnier-Pot eingefroren bleiben.[web:60][web:63]

---

## 6. End-to-End-Ablaufplan für Entwickler

Im Folgenden ein konkreter Ablaufplan für ein 5‑Team‑Turnier aus Sicht der Implementierung.

### 6.1 Vor dem Turnier

1. **Turnierkonfiguration in Kaspa Battle anlegen**
   - Admin nutzt ein internes Admin-UI oder CLI:
     - Name, Beschreibung, Spiel, Region, Format (5 Teams Single-Elimination).
     - Buy‑in, Payout-Regel, Fee-Regel.
     - FaceIT-Tournament-ID (wird nach Erstellung auf FaceIT eingetragen).

2. **Turnier in FaceIT erstellen**
   - Organizer erstellt Turnier über FaceIT (UI oder API), setzt Format/Slots.[web:74][web:76]
   - FaceIT-Tournament-ID in Kaspa Battle speichern.

3. **Covenant-Template generieren**
   - Backend ruft Covenant-Service auf und generiert ein `TournamentCovenantTemplate`:
     - Erlaubte Funding-Pattern (N Teams x Buy‑in).
     - Payout-Pattern (Top 3 + Fee).
     - Refund-Pattern (optional).
   - Ergebnis: `covenant_id` und generische Script-Vorlagen.

4. **Turnier im UI sichtbar machen**
   - Frontend zeigt Turnierliste mit On‑Chain‑Infos an (Buy‑in, Pot, Deadline, FaceIT-Link).

### 6.2 Sign‑up & Funding-Phase

1. **User verbindet FaceIT & Wallet**
   - User loggt sich via FaceIT OAuth ein.
   - User verbindet Kaspa-Wallet mit Kaspa Battle (Signatur eines Challenges).

2. **Teamregistrierung & Funding-Flow**
   - Captain wählt ein Team, das bereits bei FaceIT existiert und für das Turnier angemeldet ist.[web:72][web:74]
   - Backend validiert über FaceIT-API, dass das Team registriert ist.[web:78]
   - Backend zeigt Wallet-Adresse/QR-Code für Turnier-Funding an.

3. **Team-Funding-Transaktion**
   - Captain sendet 100 KAS von Team-Wallet an die Funding-Adresse des Turniers.
   - Wallet setzt entsprechende Covenant-Bindings (z.B. über RPC/SDK-Funktion des Covenant-Services).
   - Backend überwacht die Blockchain (Node + Indexer) und markiert Team als „Funded“, sobald die UTXO im Covenant-Set des Turniers auftaucht.

4. **Funding-Abschluss**
   - Wenn alle 5 Teams bezahlt haben oder die Funding-Deadline erreicht ist, wechselt das Turnier in den Status `Locked`.
   - Bei unvollständiger Funding-Situation kann optional ein fallback (z.B. Abbruch + Refund) ausgelöst werden.

### 6.3 Turnierlauf (Match-Phase)

1. **Bracket-Erstellung**
   - FaceIT generiert das Bracket gemäß Format.[web:76]
   - Kaspa Battle spiegelt Bracket-Struktur in eigener DB (z.B. mittels API-Poll oder Webhooks).

2. **Matches spielen**
   - Teams joinen FaceIT-Matchrooms, spielen die Matches.
   - FaceIT aktualisiert nach jedem Match den Status (Winner/Loser, nächste Runde). [web:74][web:76]

3. **Zwischenstände im Kaspa Battle UI**
   - Backend ruft periodisch oder via Webhook aktuelle Turnierdaten ab (Bracket, Standings).[web:78]
   - UI zeigt Live-Pot (On‑Chain), Bracket (FaceIT) und kombinierte Ansichten (z.B. „Winner bekommt X KAS“).

### 6.4 Turnierende & Payout

1. **Finale & Platzierungsbestimmung**
   - FaceIT markiert das Turnier als abgeschlossen und liefert eine endgültige Platzierungsübersicht (1.–3. Platz).[web:76]

2. **Payout-Vorbereitung**
   - Backend ermittelt die zugehörigen Team-Wallets für Platz 1–3 und Fee-Wallet.
   - Backend berechnet exakte Beträge (Fee + P1/P2/P3 je nach Pot und ggf. Sponsorenbeiträgen).

3. **Payout-Transaktion bauen**
   - Covenant-Service erzeugt `TxTournamentPayout` gemäß Regeln:
     - Inputs: alle Pot-UTXOs.
     - Outputs: Fee + P1 + P2 + P3.
     - Script prüft Struktur, Adressen, Summen.

4. **Signatur & Broadcast**
   - Je nach Modell gibt es verschiedene Autorisierungsvarianten:
     - a) Volle On‑Chain‑Autonomie: Covenant allein entscheidet, wenn die Bedingungen erfüllt sind (z.B. über Zeitlocks und vordefinierte Turnier-Endblöcke).
     - b) Hybrid mit Resolver: zusätzlicher Signatur-Input eines „Resolver“-Keys bestätigt, dass FaceIT-Ergebnisse final sind.
   - Nach erfolgreicher Signatur wird `TxTournamentPayout` an den Kaspa Node gesendet.

5. **Status-Update & UI**
   - Backend überwacht die Bestätigung der Payout-Transaktion.
   - UI zeigt den Abschlussstatus und die erhaltenen Beträge pro Team an.

### 6.5 Refund-Szenarien

1. **Turnier abgebrochen**
   - Organizer markiert Turnier in FaceIT als abgebrochen.
   - Backend löst `TxRefund` aus, der alle Funds an die ursprünglichen Funding-Adressen zurückschickt.

2. **Timeout / No Result**
   - Falls innerhalb eines vordefinierten Zeitfensters kein Turnierabschluss erkannt wird, kann automatisch ein Refund-Flow ausgelöst werden.

---

## 7. Sicherheits- und UX-Aspekte

### 7.1 Sicherheitsüberlegungen

- **Non-Custodial Design**  
  Covenants stellen sicher, dass weder das Kaspa Battle Backend noch einzelne Admins allein über Pot‑Gelder verfügen; sie können nur erlaubte Pfade auslösen.[web:60][web:65]

- **FaceIT als Single Source of Truth für Ergebnisse**  
  Streitigkeiten über Matchresultate werden auf FaceIT gelöst (z.B. durch Admins, Anti‑Cheat, Demo-Review). Kaspa Battle vertraut auf finalisierte Turnierdaten statt auf eigene Ergebnislogik.[web:74][web:76]

- **Resolver-Keys und Multi‑Sig**  
  Für besonders kritische Events (z.B. sehr große Pots) kann ein 2-of-3‑Scheme (Plattform + unabhängiger Schiedsrichter + Community‑Key) eingesetzt werden, um Payout-Transaktionen freizugeben.

- **Auditierbarkeit**  
  Alle Finanzflüsse finden On‑Chain statt; Dritte können Turnier-Pots und Payouts verifizieren.

### 7.2 UX-Optimierungen

- **Klare Statusanzeigen** (Funding, Locked, InProgress, ReadyForPayout, PaidOut/Refunded).
- **On‑Chain & Off‑Chain Synchronisierung** im UI (z.B. „Match gewonnen, Payout steht aus“, „Payout ausgeführt“).
- **Gas-/Fee-Abstraktion**: Plattform kann transaction building & fee estimation übernehmen, sodass Endnutzer wenig technische Details sehen.
- **Testnet-Mode**: Integration mit Testnet 12 zum risikofreien Ausprobieren von Turnieren.[web:61][web:63][web:67]

---

## 8. Erweiterungsideen für Kaspa Battle 2.0+

1. **Seasonal Ladders & Points als native Assets**  
   - Verwendung von Covenant-basierten Tokens (CAT‑ähnliche Standards) für Ligapunkte, Season‑Pässe und kosmetische Belohnungen.[web:60][web:49]

2. **Sponsoren-Pots & Bounties**  
   - Zusätzliche UTXOs für Sponsorenbeiträge, die an bestimmte In‑Game‑Events gebunden sind (z.B. „meiste Kills“, „Knife‑Ace“).

3. **ZK‑Verifikation ausgewählter Eigenschaften**  
   - In Zukunft könnte Kaspa’s ZK‑Infrastruktur genutzt werden, um bestimmte Off‑Chain‑Berechnungen (z.B. Skill‑Rating‑Δ) vertrauensarm zu bestätigen.[web:49][web:61]

4. **Cross-Platform‑Support**  
   - Erweiterung auf andere Spiele und Plattformen, solange APIs und Turniermodelle vergleichbar sind.

---

## 9. Fazit

Kaspa Battle 2.0 kombiniert **FaceITs Turnier- und Anti‑Cheat-Infrastruktur** mit **Kaspa Covenants** zu einer transparenten, non‑custodial E‑Sports‑Turnierplattform.

Das hier vorgestellte Modell für ein Turnier mit **5 Teams à 5 Spielern** dient als Blaupause für komplexere Turniere jeglicher Größe und Struktur.

Der vorgestellte Ablaufplan ist so formuliert, dass ein Entwicklungsteam unmittelbar mit der Implementierung beginnen kann: von der FaceIT‑Anbindung über die Datenmodelle und Zustandsautomaten bis hin zu konkreten On‑Chain‑Transaktionen für Funding, Payout und Refund.[web:60][web:61][web:63][web:74][web:76][web:78]
