# FaceIT-Integration, Match-Watcher und Kaspa-Multisig-Flow für KaspaBattle

## 1. Zielbild und High-Level-Flow

KaspaBattle soll künftig den kompletten Match-Lifecycle vom Lobby-Setup über FaceIT-Match-Tracking bis zur on-chain Auszahlungs-Transaktion automatisiert und betrugssicher abbilden.[^1][^2]
Die Integration umfasst drei Hauptbausteine: (1) FaceIT Data API für Match-Status, (2) Lobby-State-Maschine mit Zwischenschritt „GAME_ID_ENTERED“ und (3) Kaspa-Multisig-Signatur-Flow (Spieler + Backend/Oracle).[^3][^1]

Der Zielablauf einer Wager-Session ist:

1. Beide Spieler joinen eine KaspaBattle-Lobby und zahlen ihren Einsatz in einen Escrow (Kaspa-L1-Multisig oder L2-MatchEscrow-Contract, je nach Ausbaustufe).[^2][^1]
2. Lobby geht zunächst in Status `WAITING_FOR_GAME_ID` (oder `GAME_ID_INPUT`) statt direkt `IN_GAME`.
3. Beide Spieler erstellen bzw. joinen ein FaceIT-Match (z. B. CS2) und tragen die gleiche `match_id` in der Lobby ein.[^2][^3]
4. Sobald beide identische `match_id`s eingetragen haben, wechselt die Lobby auf `IN_GAME`, und ein FaceIT-Watcher im Backend pollt dauerhaft den Match-Status über die Data API.[^4][^3]
5. Nach Match-Ende erkennt der Watcher Ergebnis (`winner`, Scores, Players) und markiert die Lobby als `READY_FOR_PAYOUT` inkl. Gewinner-Zuordnung.[^3][^4]
6. Gewinner signiert on-chain die Auszahlungstransaktion; zusätzlich signiert das Backend (als Oracle/Schiedsrichter-Key) die Multisig-Transaktion, sodass der gesamte Pot an den Gewinner geht.[^1][^2]
7. Backend broadcastet die vollständig signierte Transaktion an das Kaspa-Netzwerk; die Lobby geht in Status `RESOLVED`.


## 2. FaceIT Data API – Grundlagen für Match-Status-Abfrage

### 2.1 Authentifizierung & API-Basis

Die FaceIT Data API (v4) ist eine REST-API, die per API-Key im HTTP-Header `Authorization: Bearer <api_key>` konsumiert wird.[^5][^6]
Sie ist serverseitig gedacht („nicht im Namen des Nutzers“), d. h. der KaspaBattle-Backend-Server hält den API-Key sicher und ruft damit Match-Informationen ab.[^6]

Wichtige Eckpunkte:

- Basis-URL: `https://open.faceit.com/data/v4`.[^4]
- Auth: `Authorization: Bearer <SERVER_SIDE_API_KEY>`.[^6][^4]
- Rate Limits: abhängig vom Plan; polling-Logik sollte konservativ (z. B. alle 10–20 Sekunden pro aktiver Lobby) gestaltet werden.


### 2.2 Relevante Endpunkte für Matches

Die Data-API-Dokumentation beschreibt mehrere Match-bezogene Endpunkte, u. a.:[^3][^4]

- `GET /matches/{match_id}` – liefert Match-Details (Status, Teams, Spieler, Start-/Endzeiten etc.).[^4][^3]
- `GET /matches/{match_id}/stats` – liefert detaillierte Match-Statistiken (Scores, Kills, Rounds etc.).[^7][^4]
- Zusätzlich indirekte Wege über Championships/Hubs, z. B.: `GET /hubs/{hub_id}/matches?type=ongoing|past`, aber für euren Flow ist der direkte `match_id`-Endpunkt zentral.[^3][^4]

Ein von der Community generierter Go-Client listet dieselben Endpunkte und bestätigt die Pfade:[^4]

- `MatchesAPI.GetMatch` → `GET /matches/{match_id}`
- `MatchesAPI.GetMatchStats` → `GET /matches/{match_id}/stats`


### 2.3 Welche IDs müssen Spieler eingeben?

FaceIT verwendet typischerweise:

- Einen globalen `match_id` (UUID-ähnlich), z. B. `18aa5180-f8a1-43e6-80aa-0fd79a090e6b`.[^7][^4]
- Der Endpunkt `GET /matches/{match_id}` akzeptiert genau diese ID.[^4]

Für KaspaBattle ist daher sinnvoll:

- In der Lobby UI wird ein Feld „FaceIT Match ID“ angezeigt.
- Beide Spieler holen die `match_id` aus der FaceIT-Match-URL (Match-Room) oder über ein optionales Browser-Plugin/Deep-Link.[^8][^5]
- Validierung im Backend: simple RegEx + Testaufruf `GET /matches/{id}` (Fehler → ungültig).

Alternative (aufwändiger, optional):

- Account-Linking: Über das offizielle Account-Integration/OAuth2-Modell kann KaspaBattle FaceIT-Accounts der Spieler verknüpfen und deren Match-History über Spieler-Endpunkte auflösen.[^9][^5]
- Dann könnte das Backend anhand des zuletzt gestarteten/aktiven Matches automatisiert die `match_id` bestimmen, statt dass Spieler sie manuell eintragen.

Für einen ersten MVP ist die manuelle Eingabe der `match_id` durch beide Spieler jedoch deutlich schneller umzusetzen und ausreichend robust.[^5]


### 2.4 Match-Status und Felder, die der Watcher braucht

Die OpenAPI-Doku für `GET /matches/{match_id}` zeigt u. a.:[^3][^4]

- `match_id`: String
- `game`: Spiel-ID (z. B. `cs2`).
- `region`, `competition_type`, `competition_id`, `organizer_id`.
- `status`: Enum-artiger String (z. B. `created`, `ongoing`, `finished` – genaue Werte stehen im Schema der Matches-Sektion).[^3][^4]
- `finished_at`: Unix-Timestamp, wenn das Match beendet ist.[^3]
- `results`: Objekt mit `winner` und ggf. Team-/Faction-Ergebnissen.
- `teams`: Objekt mit `faction1`/`faction2` (inkl. `players[*].player_id`, `game_player_name`, `nickname`).[^7][^3]

Über `GET /matches/{match_id}/stats` lassen sich zusätzliche Detailwerte holen, aber für euren Wager-Prozess reicht im Kern:[^7][^4]

- `status` → Polling-Stop-Bedingung (wenn `finished`).
- `winner` → Zuordnung auf Lobby-Teilnehmer.
- `teams[].players[].player_id` / `nickname` → Mapping der FaceIT-Identitäten zu euren Spielerprofilen.


### 2.5 Polling-Strategie („FaceIT-Watcher“)

Empfohlene Architektur für den Watcher:

- Ein Hintergrund-Task-Runner (z. B. Tokio-Task in Rust, Cron-Jobs oder dedizierter Service), der alle Lobbys mit Status `IN_GAME` oder `GAME_ID_CONFIRMED` betrachtet.
- Für jede aktive Lobby mit bekannter `match_id`:
  - Poll-Intervall: 10–20 Sekunden, konfigurierbar.[^6]
  - Request: `GET /matches/{match_id}` mit API-Key.[^4][^3]
  - Falls `status` ∈ {`finished`, `cancelled`, `aborted`}: Polling für diese Lobby stoppen und Lobby-Status entsprechend setzen:
    - `FINISHED_FACEIT` / `READY_FOR_PAYOUT` bei regulärem `finished`.
    - `CANCELLED_FACEIT` bei abgebrochenem Match → ggf. Refund-Flow.
- Optionaler zweiter Call `GET /matches/{match_id}/stats`, um zusätzliche Checks zu machen (z. B. ob beide Spieler tatsächlich teilgenommen haben, Rundenanzahl > 0 etc.).[^7]

Konfigurierbare Failover/Timeout-Regeln:

- Wenn nach X Minuten das Match noch nicht gestartet (`status` bleibt `created`/`configuring`), könnt ihr ein Timeout erzwingen und z. B. einen Refund anbieten.
- Bei API-Fehlern (429/5xx) exponential backoff und Logging.


## 3. Lobby-State-Maschine mit GAME_ID-Zwischenstatus

### 3.1 Aktueller Stand vs. Zielzustand

Laut Whitepaper und deiner Beschreibung springt der Lobby-Status aktuell direkt von „beide eingezahlt“ auf `IN_GAME`.[^2]
Ziel ist ein zusätzlicher Zwischenstatus, in dem die FaceIT-Match-ID von beiden Spielern eingegeben und abgeglichen wird.

Empfohlene State-Machine (vereinfacht):

- `OPEN` – Lobby erstellt, noch keine Einsätze.
- `FUNDING` – mindestens ein Spieler hat eingezahlt, wartet auf zweiten.
- `FUNDED` – beide Einsätze on-chain bestätigt, aber kein FaceIT-Match verknüpft.
- `GAME_ID_INPUT` – UI fordert von beiden Spielern eine FaceIT-`match_id`.
- `IN_GAME` – `match_id` von beiden Spielern bestätigt (identisch); Watcher pollt FaceIT.
- `FINISHED_FACEIT` – FaceIT meldet `finished`, Gewinner identifiziert.
- `READY_FOR_PAYOUT` – on-chain Auszahlungstransaktion vorbereitet, Signatur der Beteiligten ausstehend.
- `RESOLVED` – Auszahlung erfolgte, Match abgeschlossen.[^1][^2]


### 3.2 UI/UX-Anpassungen

In der Lobby-Ansicht:

- Nach erfolgreichem Deposit beider Spieler: Anzeige "Schritt 2: FaceIT-Match erstellen und Match-ID eintragen".
- Zwei Eingabefelder (oder ein gemeinsamer Sichtbereich):
  - Spieler A: Eingabefeld `faceit_match_id`, Button „ID speichern“.
  - Spieler B: analog.
- Feedback/Validierung:
  - Backend prüft Format + Test-Call an `GET /matches/{id}`.
  - Falls gültig: Lobby speichert `match_id_playerA` bzw. `match_id_playerB`.
  - Wenn beide gesetzt und identisch: Lobby-Status → `IN_GAME`, Feld wird read-only.

Edge Cases:

- IDs unterschiedlich: UI blendet Fehlermeldung ein („IDs stimmen nicht überein“) und erlaubt Korrektur.
- Optionaler Timeout: Wenn innerhalb einer vorgegebenen Zeit (z. B. 15 Minuten) keine übereinstimmende `match_id` vorliegt, kann die Lobby abgebrochen und Einsätze (abzüglich minimaler Gebühr) zurückerstattet werden.


### 3.3 Backend-Änderungen (Datenmodell)

Erweiterung des Lobby-Models (z. B. in Rust):

- Neue Felder:
  - `faceit_match_id_player_a: Option<String>`
  - `faceit_match_id_player_b: Option<String>`
  - `faceit_match_id_final: Option<String>`
  - `faceit_match_status: Option<String>` (z. B. `created`, `ongoing`, `finished`, `cancelled`).
  - `faceit_winner_faction_id: Option<String>`
  - `faceit_winner_player_ids: Vec<String>`
- Status-Feld erweitert um neue Enum-Werte (`FUNDED`, `GAME_ID_INPUT`, `IN_GAME`, `FINISHED_FACEIT`, `READY_FOR_PAYOUT`).

Transitions im Backend:

1. Beide Deposits bestätigt → `FUNDED`.
2. Frontend ruft API-Endpunkt `POST /lobbies/{id}/faceit-id` mit `match_id` + Auth-Spieler.
3. Backend setzt das spielerspezifische Feld; wenn beide gesetzt und identisch:
   - `faceit_match_id_final = Some(id)`.
   - Lobby-Status → `IN_GAME`.
   - Start eines Watcher-Jobs für diese Lobby.


## 4. Implementierung des FaceIT-Watchers

### 4.1 Architektur-Varianten

Mögliche technische Umsetzungen:

- Hintergrund-Worker im gleichen Backend (z. B. Rust/Tokio-Task, der periodisch alle relevanten Lobbys pollt).
- Separater Microservice „faceit-watcher“ mit eigener Queue (z. B. Kafka, Redis Streams, PostgreSQL-Job-Tabelle), der Lobbys ausliest.

Für einen MVP reicht meist ein Task im bestehenden Backend mit:

- Job-Tabelle `faceit_jobs` (Lobby-ID, match_id, next_run_at, retry_count, status).
- Periodischer Tick (z. B. alle 5 Sekunden), der Jobs mit `next_run_at <= now()` ausführt.


### 4.2 Polling-Algorithmus (Pseudo-Code)

```text
loop every 5 seconds:
  jobs = load_jobs(status IN ("IN_GAME", "POLLING"), next_run_at <= now)
  for job in jobs:
    resp = GET /matches/{job.match_id}
    if resp.status in ["finished", "cancelled", "aborted"]:
       update_lobby_faceit_state(lobby_id, resp)
       mark_job_done(job)
    else:
       job.next_run_at = now + poll_interval
       save(job)
```

`update_lobby_faceit_state` führt u. a. aus:

- Speichern von `faceit_match_status`, `finished_at` usw.
- Ermitteln des Gewinners:
  - Wenn `results.winner` auf `faction1` oder `faction2` zeigt, dann alle `players[*].player_id` dieser Faction sammeln.[^7][^3]
  - Mapping FaceIT-Player zu KaspaBattle-Spielerprofilen über gespeicherte FaceIT-IDs oder Nicknames.
- Setzen der Lobby-Felder `winner_player_id` (KaspaBattle-ID) und Status → `READY_FOR_PAYOUT`.


### 4.3 Zuordnung Winner → KaspaBattle-Spieler

Schlüsselproblem: Der Watcher muss wissen, welcher der beiden KaspaBattle-Spieler gewonnen hat.

Empfohlene Vorarbeit bei Lobby-Erstellung:

- Spielerprofil-Model enthält:
  - `faceit_player_id`
  - `faceit_nickname`
- Beim Join einer Lobby speichert ihr pro Slot (A/B) die zugehörige `faceit_player_id`.

Beim Auswerten des Matches:

- Watcher zieht aus FaceIT-Match-Details die Liste der Spieler-IDs in der Gewinner-Faction.[^3][^7]
- Schnittmenge mit den beiden `faceit_player_id`s eurer Lobby bilden:
  - Wenn genau eine ID matcht → eindeutiger Winner.
  - Wenn beide matchen (z. B. 5v5-Team-Matches) → es war ein Team; dann müsst ihr im Protokoll definieren, ob Wette team-basiert ist (z. B. „Team A vs. Team B“).

Für 1v1-Cases (CS2-Duelle) ist das Mapping trivial.


## 5. Kaspa-Multisig-Signatur-Flow für Payout

### 5.1 Aktueller Stand der Kaspa-Multisig-Fähigkeiten

Kaspa L1 nutzt ein UTXO-basiertes Scriptmodell ähnlich Bitcoin und unterstützt klassische Multisignaturen über `OP_CHECKMULTISIG` in Kombination mit P2SH (Pay-to-Script-Hash).[^1]
Ein typisches 2-aus-3-Multisig-Script sieht auf Script-Ebene vereinfacht so aus:[^1]

```text
2 <PubKeyA> <PubKeyB> <PubKeyC> 3 OP_CHECKMULTISIG
```

In der Praxis wird dieses Redeem-Script gehasht, und die Multisig-Adresse ist eine P2SH-Adresse zu diesem Hash.[^1]

Wesentliche Punkte aus der Silverskript-/Kaspa-Doku:[^10][^1]

- Multisig escrows werden erstellt, indem alle Teilnehmer ihre Beträge an eine solche P2SH-Multisig-Adresse senden.[^1]
- Zum Ausgeben (Payout) wird eine Transaktion erstellt, die den gemeinsamen UTXO als Input enthält; im `scriptSig` müssen dann mindestens M gültige Signaturen plus das Redeem-Script präsentiert werden.[^1]
- Das CLI `kaspawallet` unterstützt das Erzeugen solcher Multisig-Wallets und das sukzessive Signieren/Verteilen von Transaktionen.[^1]


### 5.2 Signaturformat für Kaspa-Transaktionen

Kaspa benutzt Schnorr-Signaturen für Standardadressen; aus Applikationssicht wird die Signierung aber vollständig über Wallet- oder SDK-Funktionen abstrahiert.[^11][^10]
Wichtige Aspekte aus der Rusty-Kaspa-Dokumentation:

- `kaspawallet` und `kaspa-wallet-core` stellen eine `WalletApi` bereit, die Funktionen wie `send`, `create-unsigned-transaction`, `sign`, `submittransaction` etc. kapselt.[^10][^11]
- Für Multisig gibt es gezeigte Abläufe, in denen:
  1. Eine unsigned TX erstellt wird (`create-unsigned-transaction`).[^1]
  2. Nacheinander mehrere Teilnehmer `kaspawallet sign` auf dieselbe TX ausführen und ihre Signatur hinzufügen.[^1]
  3. Sobald ausreichend Signaturen vorhanden sind, wird die vollständig signierte TX via `broadcast` gesendet.[^1]

Die genaue Binary-Struktur der Signatur (Schnorr über bestimmte Kurve, Serialisierung, etc.) muss im dApp-Backend nicht manuell konstruiert werden, solange ihr Rusty-Kaspa oder das WASM/TS-SDK verwendet.[^11][^10]


### 5.3 Gewünschter Flow: Spieler + Backend signieren

Euer Ziel: Gewinner und Backend signieren gemeinsam eine Multisig-Transaktion, die den Escrow-Pot an die Gewinner-Adresse auszahlt.[^2][^1]

Empfohlene Multisig-Topologie (2-aus-3):[^1]

- Keys:
  - Spieler A Public Key
  - Spieler B Public Key
  - Backend/Oracle Public Key
- Escrow-Script: `2 <PubA> <PubB> <PubBackend> 3 OP_CHECKMULTISIG`.[^1]

Ablauf pro Match:

1. Backend erstellt bei Lobby-Eröffnung eine frische Multisig-Adresse und kommuniziert diese an beide Spieler (oder generiert sie einmal pro Lobby-Typ und nutzt PSKT/pskt-Mechanismus).[^10][^1]
2. Beide Spieler senden ihren Einsatz an diese Multisig-Adresse (Kaspa L1) – entweder direkt per Wallet oder über ein L2-Smart-Contract-Äquivalent, falls ihr bereits auf Kasplex seid.[^2][^1]
3. Nach Match-Ende und FaceIT-Bestätigung konstruiert Backend eine Auszahlungstransaktion:
   - Input: UTXO des Escrows.
   - Output: Gewinner-Adresse (Kaspa P2PKH), Betrag = Pot minus Fee.
4. Backend erzeugt eine partially signed transaction (PSKT) mit eigener Signatur (Backend-Key) und sendet diese an das Wallet des Gewinners (z. B. via kaspa-auth im Browser oder per API).[^10][^1]
5. Gewinner signiert lokal im Wallet die erhaltene PSKT und schickt sie zurück oder broadcastet direkt, je nach Architektur.[^11][^1]
6. Sobald zwei Signaturen (Backend + Gewinner) vorhanden sind, ist die TX gültig und wird an das Netzwerk gesendet.[^1]


### 5.4 Konkreter Implementierungsplan für Multisig mit Rusty-Kaspa

Auf Basis der Silverskript-Dokumentation und der Rusty-Kaspa-Tech-Doku:[^10][^1]

1. **Multisig-Wallet/Adresse erzeugen**
   - Entweder über CLI:
     - `kaspawallet create --min-signatures 2 --num-private-keys 3 --num-public-keys 3`.[^1]
   - Oder programmatisch in Rust, indem die drei Extended-Pubkeys der Teilnehmer kombiniert und ein Multisig-Script gebaut wird (`crypto/txscript::standard::multisig`).[^10]

2. **Escrow-Einzahlungen**
   - Backend veröffentlicht die P2SH-Adresse in der Lobby.
   - Spieler nutzen ihre Kaspa-Wallets (oder das in euer Frontend eingebettete WASM-SDK), um an diese Adresse zu senden; das kann über `WalletApi.send` oder direkte TX-Erstellung erfolgen.[^11][^10]

3. **Payout-TX vorbereiten**
   - Backend nutzt Wallet-Core/pskt-Beispiele (`wallet/pskt/examples/multisig.rs`), um eine PSKT aus dem Escrow-UTXO zu erstellen.[^10]
   - Es fügt seine eigene Signatur hinzu.

4. **Signatur durch Gewinner**
   - Frontend ruft das Wallet/SDK des Gewinners auf, um die PSKT zu signieren.
   - `WalletApi` führt die eigentliche Signierung mit dem Private Key des Gewinners durch; die Signatur wird zur PSKT hinzugefügt.[^11][^10]

5. **Broadcast**
   - Backend oder der Client sendet die nun vollständig signierte TX über `submittransaction` an den Kaspa-Node (Rusty-Kaspa RPC).[^11][^10]

Wichtig: Auf L1 besteht „Multisig-Transaktion akzeptiert“ ausschließlich darin, dass die `scriptSig`-Daten (Signaturen + Redeem-Script) korrekt gemäß `OP_CHECKMULTISIG` aufgebaut sind; die zugrunde liegende Signaturstruktur kümmert das Wallet-Framework.[^10][^1]


### 5.5 Automatische Backend-Signatur

Das Backend benötigt einen persistenten Multisig-Schlüssel (oder mehrere Schiedsrichter-Keys), der:

- Ggf. in einem HSM, KMS oder mindestens in gut gesicherter, verschlüsselter Wallet-Storage-Struktur liegt.[^10]
- Nur für Payout-Signaturen verwendet wird, nachdem der FaceIT-Watcher das Ergebnis valide festgestellt hat.[^2][^1]

Ablauf im Backend:

1. FaceIT-Watcher markiert Lobby als `READY_FOR_PAYOUT` + Winner.
2. Backend-Layer prüft Business-Regeln (z. B. keine offenen Disputes, Orakel-Konsens ausreichend) – im L1-Hybrid-Modell kann das rein Off-Chain geschehen, im L2-Modell ggf. im Smart Contract.[^2][^1]
3. Backend erstellt eine PSKT, signiert sie mit seinem Key und stellt sie dem Gewinner zur Verfügung.
4. Optional: Logging aller Signaturvorgänge (Audit-Trail).


## 6. End-to-End-Prozessbeschreibung (Doku-Blueprint)

### 6.1 Phasenübersicht

Der vollständige Ablauf lässt sich in fünf Phasen gliedern:[^2][^1]

1. **Lobby & Escrow-Setup**
   - Lobby-Erstellung (Spiel, Einsatz, Regeln, Timeout).
   - Multisig-Escrow-Adresse generieren.
   - Beide Spieler zahlen in den Escrow ein → Lobby-Status `FUNDED`.

2. **FaceIT-Match-Verknüpfung**
   - Spieler erstellen/joinen FaceIT-Match.
   - Beide tragen `match_id` ein; Backend validiert ID und Gleichheit.
   - Lobby-Status → `GAME_ID_INPUT` → `IN_GAME`.

3. **Match-Tracking via FaceIT-Watcher**
   - Polling des Endpunkts `GET /matches/{match_id}` bis `status = finished`.
   - Optional: zusätzliche Konsistenzprüfungen über `/stats`.[^7][^3]
   - Gewinner-Bestimmung über Mapping FaceIT-IDs ↔ KaspaBattle-Profile.
   - Lobby-Status → `FINISHED_FACEIT` → `READY_FOR_PAYOUT`.

4. **Multisig-Payout-Signaturen**
   - Backend konstruiert PSKT mit Payout an Gewinner.
   - Backend signiert mit eigenem Multisig-Key.
   - Gewinner signiert via Wallet/SDK.
   - Final signierte TX wird über Rusty-Kaspa RPC gebroadcastet.[^11][^10]

5. **Abschluss & Historie**
   - Lobby-Status → `RESOLVED`.
   - On-Chain-TX-Hash + FaceIT-Match-Daten werden in der Match-History gespeichert.


### 6.2 Empfohlene Schnittstellen (API-Design)

Beispielhafte REST-Endpoints für KaspaBattle-Backend:

- `POST /lobbies` – neue Lobby erstellen.
- `POST /lobbies/{id}/deposit` – Deposit-Callback (nach on-chain Confirmation oder direkt vom Wallet-Service getriggert).
- `POST /lobbies/{id}/faceit-id` – FaceIT-`match_id` speichern (pro Spieler).
- `GET /lobbies/{id}/status` – aktuellen Lobby-/Matchstatus für Frontend.
- `POST /lobbies/{id}/payout/sign` – (optional) PSKT vom Gewinner signieren, falls nicht direkt über Wallet-SDK.
- `POST /lobbies/{id}/payout/broadcast` – final signierte TX an Node senden.


### 6.3 Fehler- und Dispute-Handling

Dispute-Fälle laut Whitepaper und Silverskript-Doku:[^2][^1]

- FaceIT-Match abgebrochen oder nicht gestartet → Timeout & Refund.
- FaceIT-API nicht erreichbar → temporärer Fehler, Retry mit Backoff; nach definiertem Max-Zeitfenster manueller Dispute-Prozess.
- Abweichung zwischen FaceIT-Ergebnis und Spieler-Erwartung → optionaler Off-Chain-Schiedsprozess (DAO/Arbitration in späterer Phase).


## 7. Konkrete Next Steps für die Implementierung

1. **FaceIT-API-Zugang einrichten**
   - Registrierung im FaceIT Developer Portal.
   - Server-Side API Key erzeugen und sicher im Backend speichern.[^5][^6]

2. **Lobby-State-Maschine erweitern**
   - Neue Enum-Werte implementieren (`FUNDED`, `GAME_ID_INPUT`, `IN_GAME`, `FINISHED_FACEIT`, `READY_FOR_PAYOUT`).
   - Datenmodell um `faceit_match_id_*` und Ergebnisfelder ergänzen.

3. **FaceIT-`match_id`-Inputflow bauen**
   - Frontend-Formular + Validierungs-Endpunkt.
   - Backend-Logik: IDs pro Spieler speichern, Gleichheit prüfen, Test-Request gegen `GET /matches/{id}`.

4. **Watcher-Service implementieren**
   - Job-Tabelle/Queue entwerfen.
   - Polling-Loop mit konfigurierbarem Intervall.
   - Parser für `GET /matches/{match_id}`-Response implementieren und auf Status/Ergebnisse mappen.[^4][^3]

5. **Mapping FaceIT-Spieler ↔ KaspaBattle-User**
   - Spielerprofil um FaceIT-IDs ergänzen.
   - Beim Lobby-Join FaceIT-ID der beiden Spieler speichern.
   - Matching-Logik im Watcher implementieren.

6. **Kaspa-Multisig-Flow integrieren**
   - Basierend auf Silverskript-Doku ein 2-aus-3-Escrow-Konzept umsetzen.[^1]
   - Rusty-Kaspa Wallet- oder PSKT-Beispiele als Grundlage für PSKT-Erzeugung und -Signierung nutzen.[^10]
   - Backend-Key-Management (Schiedsrichter-Key) sicher aufsetzen.

7. **UX für Gewinner-Signatur**
   - Im Frontend: Nach `READY_FOR_PAYOUT` Wallet-Prompt anzeigen („Gewinn auszahlen – Transaktion signieren“).
   - Wallet-Integration via kaspa-auth/WASM-SDK, damit die Signatur clientseitig und non-custodial erfolgt.[^2][^10]

8. **End-to-End-Tests im Testnet**
   - Kaspa-Testnet-Node (Rusty-Kaspa) aufsetzen und mit `--utxoindex` betreiben.[^11][^10]
   - Dummy-FaceIT-API-Keys oder Sandbox-Umgebung nutzen.
   - Komplette Flows durchspielen: Deposit → GAME_ID_INPUT → IN_GAME → FINISHED_FACEIT → READY_FOR_PAYOUT → RESOLVED.


## 8. Zusammenfassung der wichtigsten Antworten auf deine Fragen

- **Benötigte FaceIT-Daten für den Watcher:**
  - Primär `match_id`, `status`, `results.winner`, `teams[].players[]` aus `GET /matches/{match_id}`; optional Stats aus `/stats`.[^3][^7]
- **Was müssen Spieler eingeben?**
  - Die FaceIT-`match_id` (aus der Match-URL oder Matchroom UI); beide müssen dieselbe ID eingeben.[^7][^4]
- **Wie dauerhaft den Spielstatus abfragen?**
  - Per Backend-Watcher, der in festen Intervallen `GET /matches/{match_id}` aufruft, bis `status = finished` (oder Fehler/Timeout).[^4][^3]
- **Wie muss die Signatur für Kaspa-Multisig aussehen?**
  - Praktisch wird die Signatur durch Rusty-Kaspa Wallet/SDK generiert; technisch handelt es sich um M von N Schnorr-Signaturen in einem `OP_CHECKMULTISIG`-Script, verpackt in P2SH.[^10][^1]
- **Wie signiert das Backend automatisch?**
  - Durch Halten eines eigenen Multisig-Keys im Wallet-Framework und automatisches Signieren der PSKT, sobald der FaceIT-Watcher ein valides Ergebnis meldet.[^10][^1]

Dieses Dokument kann direkt als Implementierungs-Blueprint für die nächsten Sprints im KaspaBattle-Projekt dienen, insbesondere für die FaceIT-Integration und den Multisig-Auszahlungs-Flow.

---

## References

1. [Kaspa-Silverskript.md](https://ppl-ai-file-upload.s3.amazonaws.com/web/direct-files/collection_ca0bb9cb-01a1-41cb-af58-b7603a992523/4ed98bff-1d1b-4102-8714-7383b026b399/Kaspa-Silverskript.md?AWSAccessKeyId=ASIA2F3EMEYE4WCWA4S2&Signature=8WUmvRF2rwZDPuNMm%2FCZ8XUoWFg%3D&x-amz-security-token=IQoJb3JpZ2luX2VjEDcaCXVzLWVhc3QtMSJGMEQCIAEheOF26Nv8Xr9nnL0DxEDwC0PhPDzaiY0MsQf%2B6976AiB2091s5PW0FbNFLAc8vTf11uApnAdk2mbVVSGo1SMItSrzBAgAEAEaDDY5OTc1MzMwOTcwNSIM1awjb1zZVyEiHTQgKtAEuf7s9QF5CQ7%2Bhhc2xGeyDwu8taZPXEdWKQs52POu4fvrOWktAeHlBPjCZxQ39JIQbzOp12y7ovc3Uujc9zebJN9rmeCs%2FPYjKg8Oeq%2BF4pDFBFkSS50oJSMAAZuCjh2awFlktwOmF%2BD3SSsgnDGntrrvUNKUyZfM3%2FHNzbH%2FHeWNtzF9Bf5Cj%2BxdpnPuovnpGEFTzRZcG3McrxulbdcqpN9n79T0y2mMT2%2BZY4WABnh%2FyMPzcEHJ%2F87Y2ye1SqzGIDWfDl%2FVHEN4NvPJ52fRdznYgHE0xGmo49nhIjp2gBb3jXFA0T%2BhreMwUNfjvuDrCXzWL%2FvhyDsdLExhdHVCeNMcXfqxL3mEadanE4Pep%2FIWH7lZ8D2njbo8mtIEXlMU4%2BqElVWAfpdy9yBHXysRY3DbAGd2TsHT1gia%2FxZl9B8e3M3xcdSi9lzb08CLKzhEf8Lm21a79PCYKuKHHpV075DK7Nv7dUKN2jExvVUaetZUYtL1tjdfcNXmNnwed8TvUofjUFA0RerHZSXyX%2BfYOxtZjFbvtNWV4%2FFNupPSmpcTRMR4mO774VR0x6IybZhRdMO7QW8svymKz3CwDeOrcuy70dfVas3MYCs2QRqZmqylTrkdLaZirBKfF1qBYnu2FurWf66ktTd9PYAo23wfHDaWCtRa7%2FQWSNF80Smd8wBk32E0NDMIRwbncOVt3bQ7QZUnRBA3BcTaIhhDzxxvNiP9JxkAFysT9LaXmFmE0yDLJvIsr83Z5NRCx8j0HN05BTC2sHGGHueqY7FP9CAzoTCRraHOBjqZAfAtD8LsnZbF289BxOXVZL4E5hvxYAKY1Ept3INTmlVajE85PZqddIzIpc6TDPjbyPFhY46mSjE%2FRjO%2BKZ%2BqNGZwFHoi24gnv3OHjLU%2BHZ3RAyDl2FUl7ieKuuVMYrNLTxYR54RluqF1dJpsnby9Uq4WUjhwUej4DhvLtgwLOW5vAtczESLsmlJ1Rzz%2BOd0yVDOCc98N%2BJrcoQ%3D%3D&Expires=1774740580) - Kaspa ist derzeit nicht mit Turing-vollstndigen Smart Contracts auf Layer 1 L1 ausgestattet. Stattde...

2. [KaspaBattle-Whitepaper.docx](https://ppl-ai-file-upload.s3.amazonaws.com/web/direct-files/collection_ca0bb9cb-01a1-41cb-af58-b7603a992523/0d6afe20-b6ed-424f-9feb-1699928a32c9/KaspaBattle-Whitepaper.docx?AWSAccessKeyId=ASIA2F3EMEYE4WCWA4S2&Signature=2Vt5iuV3AnsE3oNxQJMlRFxEElM%3D&x-amz-security-token=IQoJb3JpZ2luX2VjEDcaCXVzLWVhc3QtMSJGMEQCIAEheOF26Nv8Xr9nnL0DxEDwC0PhPDzaiY0MsQf%2B6976AiB2091s5PW0FbNFLAc8vTf11uApnAdk2mbVVSGo1SMItSrzBAgAEAEaDDY5OTc1MzMwOTcwNSIM1awjb1zZVyEiHTQgKtAEuf7s9QF5CQ7%2Bhhc2xGeyDwu8taZPXEdWKQs52POu4fvrOWktAeHlBPjCZxQ39JIQbzOp12y7ovc3Uujc9zebJN9rmeCs%2FPYjKg8Oeq%2BF4pDFBFkSS50oJSMAAZuCjh2awFlktwOmF%2BD3SSsgnDGntrrvUNKUyZfM3%2FHNzbH%2FHeWNtzF9Bf5Cj%2BxdpnPuovnpGEFTzRZcG3McrxulbdcqpN9n79T0y2mMT2%2BZY4WABnh%2FyMPzcEHJ%2F87Y2ye1SqzGIDWfDl%2FVHEN4NvPJ52fRdznYgHE0xGmo49nhIjp2gBb3jXFA0T%2BhreMwUNfjvuDrCXzWL%2FvhyDsdLExhdHVCeNMcXfqxL3mEadanE4Pep%2FIWH7lZ8D2njbo8mtIEXlMU4%2BqElVWAfpdy9yBHXysRY3DbAGd2TsHT1gia%2FxZl9B8e3M3xcdSi9lzb08CLKzhEf8Lm21a79PCYKuKHHpV075DK7Nv7dUKN2jExvVUaetZUYtL1tjdfcNXmNnwed8TvUofjUFA0RerHZSXyX%2BfYOxtZjFbvtNWV4%2FFNupPSmpcTRMR4mO774VR0x6IybZhRdMO7QW8svymKz3CwDeOrcuy70dfVas3MYCs2QRqZmqylTrkdLaZirBKfF1qBYnu2FurWf66ktTd9PYAo23wfHDaWCtRa7%2FQWSNF80Smd8wBk32E0NDMIRwbncOVt3bQ7QZUnRBA3BcTaIhhDzxxvNiP9JxkAFysT9LaXmFmE0yDLJvIsr83Z5NRCx8j0HN05BTC2sHGGHueqY7FP9CAzoTCRraHOBjqZAfAtD8LsnZbF289BxOXVZL4E5hvxYAKY1Ept3INTmlVajE85PZqddIzIpc6TDPjbyPFhY46mSjE%2FRjO%2BKZ%2BqNGZwFHoi24gnv3OHjLU%2BHZ3RAyDl2FUl7ieKuuVMYrNLTxYR54RluqF1dJpsnby9Uq4WUjhwUej4DhvLtgwLOW5vAtczESLsmlJ1Rzz%2BOd0yVDOCc98N%2BJrcoQ%3D%3D&Expires=1774740580) - KaspaBattle Eine dezentrale Peer-to-Peer Competitive Gaming Wager-Plattform auf Kaspa Version 1.0 Fe...

3. [Docs - FACEIT for Developers](https://docs.faceit.com/docs/data-api/data) - This API provide access to FACEIT's data. Championships Retrieve all championships of a game Retriev...

4. [mconnat/go-faceit - GitHub](https://github.com/mconnat/go-faceit) - Documentation for API Endpoints. All URIs are relative to https://open.faceit.com/data/v4 ... Retrie...

5. [Retrieving FACEIT Data - FACEIT for Developers](https://docs.faceit.com/getting-started/Guides/retreiving-faceit-data) - We made FACEIT data available through the FACEIT Data API. This means that all data from players, te...

6. [Data API - FACEIT for Developers](https://docs.faceit.com/docs/data-api/) - The Data API are used programmatically not on behalf of the user. This means that you can use one of...

7. [GitHub - kallelundgren93/faceit-ruby: This gem is for the data v4 api ...](https://github.com/kallelundgren93/faceit-ruby) - This is a implementation for most of the endpoints the Faceit Data v4 api offers, feel free to contr...

8. [FACEIT Demo Upload API Endpoint - Leetify](https://leetify.com/blog/faceit-demo-upload-api/) - Use this to automate the process of opening the FACEIT match room, downloading the demo file, and up...

9. [Account Linkage | FACEIT for Developers](https://docs.faceit.com/getting-started/partner-integration-guide/account-linkage/) - Account Linkage. Account linkage flow​. Account linkage is the mechanism that allows FACEIT and part...

10. [TECHNISCHE_DOKUMENTATION.md](https://ppl-ai-file-upload.s3.amazonaws.com/web/direct-files/collection_ca0bb9cb-01a1-41cb-af58-b7603a992523/7c333415-f732-48eb-8cd5-f5f813b6c788/TECHNISCHE_DOKUMENTATION.md?AWSAccessKeyId=ASIA2F3EMEYE4WCWA4S2&Signature=GRmiM9IYUldi7Du0EzTMTxzGbLE%3D&x-amz-security-token=IQoJb3JpZ2luX2VjEDcaCXVzLWVhc3QtMSJGMEQCIAEheOF26Nv8Xr9nnL0DxEDwC0PhPDzaiY0MsQf%2B6976AiB2091s5PW0FbNFLAc8vTf11uApnAdk2mbVVSGo1SMItSrzBAgAEAEaDDY5OTc1MzMwOTcwNSIM1awjb1zZVyEiHTQgKtAEuf7s9QF5CQ7%2Bhhc2xGeyDwu8taZPXEdWKQs52POu4fvrOWktAeHlBPjCZxQ39JIQbzOp12y7ovc3Uujc9zebJN9rmeCs%2FPYjKg8Oeq%2BF4pDFBFkSS50oJSMAAZuCjh2awFlktwOmF%2BD3SSsgnDGntrrvUNKUyZfM3%2FHNzbH%2FHeWNtzF9Bf5Cj%2BxdpnPuovnpGEFTzRZcG3McrxulbdcqpN9n79T0y2mMT2%2BZY4WABnh%2FyMPzcEHJ%2F87Y2ye1SqzGIDWfDl%2FVHEN4NvPJ52fRdznYgHE0xGmo49nhIjp2gBb3jXFA0T%2BhreMwUNfjvuDrCXzWL%2FvhyDsdLExhdHVCeNMcXfqxL3mEadanE4Pep%2FIWH7lZ8D2njbo8mtIEXlMU4%2BqElVWAfpdy9yBHXysRY3DbAGd2TsHT1gia%2FxZl9B8e3M3xcdSi9lzb08CLKzhEf8Lm21a79PCYKuKHHpV075DK7Nv7dUKN2jExvVUaetZUYtL1tjdfcNXmNnwed8TvUofjUFA0RerHZSXyX%2BfYOxtZjFbvtNWV4%2FFNupPSmpcTRMR4mO774VR0x6IybZhRdMO7QW8svymKz3CwDeOrcuy70dfVas3MYCs2QRqZmqylTrkdLaZirBKfF1qBYnu2FurWf66ktTd9PYAo23wfHDaWCtRa7%2FQWSNF80Smd8wBk32E0NDMIRwbncOVt3bQ7QZUnRBA3BcTaIhhDzxxvNiP9JxkAFysT9LaXmFmE0yDLJvIsr83Z5NRCx8j0HN05BTC2sHGGHueqY7FP9CAzoTCRraHOBjqZAfAtD8LsnZbF289BxOXVZL4E5hvxYAKY1Ept3INTmlVajE85PZqddIzIpc6TDPjbyPFhY46mSjE%2FRjO%2BKZ%2BqNGZwFHoi24gnv3OHjLU%2BHZ3RAyDl2FUl7ieKuuVMYrNLTxYR54RluqF1dJpsnby9Uq4WUjhwUej4DhvLtgwLOW5vAtczESLsmlJ1Rzz%2BOd0yVDOCc98N%2BJrcoQ%3D%3D&Expires=1774740580) - Version 1.0 Datum Februar 2026 Repository httpsgithub.comkaspanetrusty-kaspa --- TITLE Rusty-Kaspa U...

11. [QUICK_START_GUIDE.md](https://ppl-ai-file-upload.s3.amazonaws.com/web/direct-files/collection_ca0bb9cb-01a1-41cb-af58-b7603a992523/2b557dba-9263-4fc9-ab94-46694ab50b36/QUICK_START_GUIDE.md?AWSAccessKeyId=ASIA2F3EMEYE4WCWA4S2&Signature=tM4y%2FV8I4%2F6vrVs%2Bsh7NNnLeFPI%3D&x-amz-security-token=IQoJb3JpZ2luX2VjEDcaCXVzLWVhc3QtMSJGMEQCIAEheOF26Nv8Xr9nnL0DxEDwC0PhPDzaiY0MsQf%2B6976AiB2091s5PW0FbNFLAc8vTf11uApnAdk2mbVVSGo1SMItSrzBAgAEAEaDDY5OTc1MzMwOTcwNSIM1awjb1zZVyEiHTQgKtAEuf7s9QF5CQ7%2Bhhc2xGeyDwu8taZPXEdWKQs52POu4fvrOWktAeHlBPjCZxQ39JIQbzOp12y7ovc3Uujc9zebJN9rmeCs%2FPYjKg8Oeq%2BF4pDFBFkSS50oJSMAAZuCjh2awFlktwOmF%2BD3SSsgnDGntrrvUNKUyZfM3%2FHNzbH%2FHeWNtzF9Bf5Cj%2BxdpnPuovnpGEFTzRZcG3McrxulbdcqpN9n79T0y2mMT2%2BZY4WABnh%2FyMPzcEHJ%2F87Y2ye1SqzGIDWfDl%2FVHEN4NvPJ52fRdznYgHE0xGmo49nhIjp2gBb3jXFA0T%2BhreMwUNfjvuDrCXzWL%2FvhyDsdLExhdHVCeNMcXfqxL3mEadanE4Pep%2FIWH7lZ8D2njbo8mtIEXlMU4%2BqElVWAfpdy9yBHXysRY3DbAGd2TsHT1gia%2FxZl9B8e3M3xcdSi9lzb08CLKzhEf8Lm21a79PCYKuKHHpV075DK7Nv7dUKN2jExvVUaetZUYtL1tjdfcNXmNnwed8TvUofjUFA0RerHZSXyX%2BfYOxtZjFbvtNWV4%2FFNupPSmpcTRMR4mO774VR0x6IybZhRdMO7QW8svymKz3CwDeOrcuy70dfVas3MYCs2QRqZmqylTrkdLaZirBKfF1qBYnu2FurWf66ktTd9PYAo23wfHDaWCtRa7%2FQWSNF80Smd8wBk32E0NDMIRwbncOVt3bQ7QZUnRBA3BcTaIhhDzxxvNiP9JxkAFysT9LaXmFmE0yDLJvIsr83Z5NRCx8j0HN05BTC2sHGGHueqY7FP9CAzoTCRraHOBjqZAfAtD8LsnZbF289BxOXVZL4E5hvxYAKY1Ept3INTmlVajE85PZqddIzIpc6TDPjbyPFhY46mSjE%2FRjO%2BKZ%2BqNGZwFHoi24gnv3OHjLU%2BHZ3RAyDl2FUl7ieKuuVMYrNLTxYR54RluqF1dJpsnby9Uq4WUjhwUej4DhvLtgwLOW5vAtczESLsmlJ1Rzz%2BOd0yVDOCc98N%2BJrcoQ%3D%3D&Expires=1774740580) - Ein praktischer Guide mit Copy-Paste-Ready Code-Beispielen fr schnelle Integration. --- TITLE Rusty-...

