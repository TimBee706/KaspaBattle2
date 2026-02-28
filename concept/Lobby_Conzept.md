# KaspaBattle Lobby-Integration Konzept

Du bist ein **Senior Full‑Stack‑Architekt** mit Expertise in **Rust (kdapp-Framework), React/TypeScript, Kaspa WASM-SDK, Kasplex L2 Smart Contracts, FACEIT API** und **Test-Driven Development**.

1. Annahmen und Ziele
Frontend ist eine Single‑Page‑App (React/TypeScript, optional Tauri) und nutzt bereits kaspa-auth / WASM‑SDK für die Wallet‑Verbindung.
​

Backend ist ein Rust‑Service auf Basis des kdapp‑Frameworks, der Match‑Episoden verwaltet und mit Smart Contracts (MatchEscrow, OracleRouter) auf Kasplex L2 spricht.
​

Ziel: Eine Lobby‑Schicht, die

offene Challenges (Lobbys) anzeigt,

neue Challenges erstellen lässt,

Join/Beitreten sauber mit Wallet‑Deposits und FACEIT verknüpft
und später einen Verlauf aus on‑chain/verifizierten Resultaten ableitet.
​

1. Datenmodell & Statusdefinition
2.1 Off‑Chain Datenbank‑Modelle
Lege in PostgreSQL (oder deiner bestehenden DB) u. a. folgende Tabellen an:
​

users

id, kaspa_address, faceit_id, nickname, created_at.

matches (Lobby/Challenge)

id (UUID), onchain_match_id (uint256, nullable),
creator_user_id, opponent_user_id (nullable solange offen),
game_id (z. B. "cs2"), platform ("FACEIT"),
stake_kas (pro Spieler), mode ("BO1" | "BO3"),
status ("OPEN" | "AWAITING_FUNDING" | "LOCKED" | "IN_GAME" | "RESOLVED" | "CANCELLED"),
external_match_id (FACEIT Match‑ID, nullable bis Erstellung),
created_at, updated_at.
​

match_results (History/Verlauf, später)

match_id, winner_user_id, onchain_tx_hash_payout, oracle_proof_hash, resolved_at.
​

2.2 Mapping zu Smart‑Contract‑Status
Mappe deine Off‑Chain‑Status direkt auf den MatchState im MatchEscrow‑Contract:
​

OPEN ↔ MatchState.OPEN

AWAITING_FUNDING/LOCKED ↔ FUNDED/LOCKED

RESOLVED ↔ RESOLVED mit Winner + Auszahlung.

So stellst du sicher, dass Lobby‑Anzeige, Backend‑Logik und On‑Chain‑Zustand konsistent bleiben und keine Funktion „verloren geht“.
​

1. Frontend: Navigation & Routen
3.1 Hauptseite und Routing
Behalte deine drei Buttons auf der Startseite bei:

„Lobby“ → Route /lobby

„Challenge erstellen“ → öffnet Modal oder Route /challenge/new

„Verlauf“ → Route /history (zunächst minimal, aber Routing direkt definieren).
​

Implementiere einen zentralen Layout‑Container (MainLayout), der Wallet‑Status (connected/disconnected) und FACEIT‑Status (linked/unlinked) im Header zeigt, um dem User immer Kontext zu geben.
​

3.2 Globaler Frontend‑State
In einem Store (z. B. Zustand/Redux):

currentUser: kaspaAddress, faceitId, nickname.

lobbies: Liste aller aktuell relevanten Matches (siehe 4.2).

activeMatch: wenn der User gerade in einer Challenge steckt.

history: persönliche Match‑History (für später).
​

1. Frontend: Lobby-Übersicht (/lobby)
4.1 UI‑Layout
Tabelle oder Card‑Grid mit Spalten/Infos:

Gegner (Ersteller der Lobby), Spiel, Einsatz (KAS pro Spieler), Modus (BO1/BO3), Plattform (FACEIT), Status, Aktion.
​

Klar getrennte Sektionen:

„Offene Lobbys“ (Status = OPEN, bei denen der aktuelle User noch kein Teilnehmer ist).

„Meine Lobbys“ (alle Matches, an denen der User beteiligt ist, egal ob offen oder schon funded/locked).
​

4.2 Datenbeschaffung
Beim Aufruf von /lobby:

GET /api/lobbies?scope=all → gibt zwei Arrays zurück: openLobbies, myLobbies (siehe Backend 7.1).
​

Optional: WebSocket/Event‑Stream

Kanal lobby_updates für neue/aktualisierte Lobbys (z. B. wenn jemand eine Lobby erstellt oder beitritt), damit die Übersicht „live“ ist.
​

4.3 Aktionen in der Übersicht
Für eine offene Lobby eines anderen Spielers: Button „Beitreten“.

Für eigene offene Lobbys: Button „Abbrechen“ (setzt Status auf CANCELLED und ggf. ruft Contract‑Funktion claimTimeout oder Off‑Chain‑Cancel auf, solange noch keine Einzahlungen erfolgt sind).
​

1. Frontend: Challenge-Erstellung (Modal oder /challenge/new)
5.1 UI-Felder und Validierung
Modal/Seite mit:

Dropdown „Spiel“ (Liste der unterstützten Titel, z. B. CS2/Valorant).

Input „Einsatz (KAS)“: numerisch, validiere gegen Min/Max aus Whitepaper (z. B. 10–10.000 KAS).
​

Radio „Match‑Modus“: Best‑of‑One (BO1) oder Best‑of‑Three (BO3).

Client‑seitige Checks:

Wallet verbunden (sonst Hinweis + Button „Wallet verbinden“).

FACEIT verknüpft (falls nicht, direkt Link‑Flow starten, aber du hast das ja bereits umgesetzt).

5.2 Ablauf beim Erstellen
User klickt „Challenge erstellen“.

Frontend öffnet Modal, User füllt Felder aus.

Klick auf „Challenge erstellen“ triggert POST /api/challenges mit { gameId, stakeKas, mode }.
​

Backend erstellt zunächst einen DB‑Eintrag (matches.status = OPEN) und optional eine kdapp‑Episode mit Initial‑Parametern (ohne on-chain‑Escrow, wenn du on-chain erst beim Funding triggern willst).
​

API‑Antwort enthält die neue Lobby (matchId), die Frontend sofort in „Meine Lobbys“ einblendet.

Damit ist sichergestellt, dass die Lobby direkt sichtbar ist, auch bevor die erste on‑chain Einzahlungs‑Transaktion durch ist.
​

1. Frontend: Beitreten zu einer Lobby
6.1 UX-Flow beim Klick „Beitreten“
User klickt in /lobby auf eine offene Lobby eines anderen Spielers.

Bestätigungs‑Modal:

Zeige Spiel, Einsatz, Modus, FACEIT‑Plattform und geschätzte Protokollgebühr (5 % gesamt, also 2,5 % pro Spieler – nur zur Info).
​

Klick „Beitreten & KAS hinterlegen“.

6.2 Technische Schritte im Frontend
Call POST /api/challenges/{matchId}/join.

Backend gibt dir zurück:

Infos zur zu signierenden Deposit‑Transaktion (z. B. prepared unsigned TX) oder onchain_match_id plus Betrag und Contract‑Adresse, falls der Deposit rein über das Wallet/SDK gebaut wird.
​

Nutze dein bestehendes kaspa Wallet SDK / kaspa-auth, um die Transaktion zu signieren und zu broadcasten (Frontend‑Flow kennst du schon aus der Wallet‑Integration).
​

Nach Erfolg:

Zeige Status „Deposit gesendet – warte auf Bestätigung“.

Poll/Subscribe auf Backend‑Status für dieses Match oder auf Contract‑Events, bis Status LOCKED ist.
​

Wenn sowohl Spieler A (Ersteller) als auch Spieler B ihren Deposit abgeschlossen haben, wechselt die Lobby automatisch in den Bereich „Aktive Matches“ (oder wird im Lobby‑Screen entsprechend markiert).
​

1. Backend/API: Services & Endpunkte
7.1 REST-/RPC-Endpunkte
Definiere u. a.:

GET /api/lobbies

Query‑Parameter: scope=open|mine|all.

Rückgabe: offene Lobbys + eigene Lobbys inkl. Status und Basis‑Infos.
​

POST /api/challenges

Body: { gameId, stakeKas, mode }.

Logik: erstellt DB‑Match + kdapp‑Episode (Status OPEN).
​

POST /api/challenges/{id}/join

Prüft: User ist nicht Ersteller, Match ist OPEN.

Legt opponent_user_id fest und initiiert on‑chain‑Deposit‑Flow (siehe 8).
​

GET /api/challenges/{id}

Detail‑Ansicht für Frontend (für Match‑Seite/Modal).

Schon jetzt kannst du GET /api/history vordefinieren, auch wenn das Frontend „Verlauf“ erst später voll nutzt.
​

7.2 kdapp-Integration
Jedes Match ist eine „Episode“ im kdapp‑Framework mit:

Initial‑State: Teilnehmer‑Public‑Keys, Stake, Game‑ID, Modus.
​

execute‑Aufrufe repräsentieren State‑Übergänge:

Create → Episode angelegt (Status OPEN).

Fund → Deposits beider Spieler bestätigt (Status LOCKED).

Resolve → Resultat eingetragen und Auszahlung erfolgt (Status RESOLVED).
​

So bekommst du automatische Rollbacks bei Reorgs und einen konsistenten State fürs Frontend.
​

1. Backend: Kaspa Wallet & Smart Contracts
8.1 Challenge-Erstellung vs. Deposit
Du hast zwei vernünftige Modelle, beide kompatibel mit deinem Whitepaper:
​

On‑Chain bereits bei Erstellung:

Ersteller ruft createMatch(wagerAmount, gameId, externalMatchId?) im MatchEscrow‑Contract auf, Contract‑Status = OPEN.
​

Joiner ruft acceptMatch(matchId) mit seinem Deposit auf, danach MatchState → FUNDED/LOCKED.

On‑Chain erst beim Funding:

Backend erstellt nur Off‑Chain‑Lobby, on‑chain wird erst beim ersten Deposit (Ersteller oder Beitretender) angelegt.

Empfehlung (vereinfachte UX):

Ersteller und Beitretender funden jeweils bei ihrem ersten „Commit“ (Ersteller beim Erstellen, Joiner beim Beitreten), so wie im vollständigen Match‑Zyklus im Whitepaper beschrieben.
​

8.2 Konkreter Backend-Flow beim Erstellen
POST /api/challenges

Backend erstellt DB‑Match + kdapp‑Episode.

Backend generiert die Parameter für createMatch (wagerAmount, gameId, ggf. leere externalMatchId).

Antwort an Frontend:

matchId, unsignedTx oder contractCallData + Zieladresse.

Frontend signiert mit Kaspa‑Wallet; nach Bestätigung hört Backend auf Contract‑Event MatchCreated, speichert onchain_match_id im DB‑Match und setzt Status AWAITING_FUNDING (oder lässt vorerst OPEN, bis beide funded sind).
​

1. Backend: FACEIT & Oracle-Integration
9.1 FACEIT-Match-Erstellung
Sobald beide Spieler LOCKED sind (Escrow funded), triggert Backend:

POST /faceit/matches (über deine bestehende FACEIT‑Integration) mit beiden FACEIT‑Accounts und Spiel‑Settings.
​

Antwort enthält external_match_id (FACEIT Match‑ID), die im Match‑Datensatz gespeichert und im Contract (per updateExternalMatchId oder direkt bei createMatch) referenziert wird.
​

9.2 Ergebnisverarbeitung
Oracle‑Nodes fragen FACEIT Data API nach dem Ergebnis (Sieger, Score etc.) ab.
​

OracleRouter‑Contract aggregiert die Oracle‑Signaturen und ruft resolveMatch(matchId, winner, proof) im MatchEscrow‑Contract.
​

Backend lauscht auf MatchResolved‑Event:

aktualisiert matches.status = RESOLVED,

setzt winner_user_id in match_results und speichert Proof/Tx‑Hash.
​

Frontend kann per Poll/WebSocket informiert werden („Match beendet – Gewinner: …“).

Damit stellst du sicher, dass Verlauf und Lobby‑Status immer mit den verifizierten On‑Chain‑Daten übereinstimmen.
​

1. Frontend: Verlauf (Grundlage jetzt legen)
Auch wenn „Verlauf“ erst später wichtig ist, solltest du die Pipeline jetzt korrekt definieren.

GET /api/history?limit=…&offset=…

Liefert alle abgeschlossenen Matches (status = RESOLVED) für den aktuellen User inkl. Einsatz, Spiel, Modus, Ergebnis, on‑chain‑TX‑Hashes.
​

Frontend‑Route /history zeigt einfache Liste/Karten:

Gewinn/Verlust, Datum, Game, Stake, Link zu Explorer (On‑Chain‑TX).

So verlierst du keine Information: alles, was im Lobby‑Kontext passiert, wandert nach „RESOLVED“ automatisch in den Verlauf.
​

1. Sichtbarkeit & Rollenlogik (wer sieht was?)
Damit „alle Informationen passend für jeden Benutzer aufrufbar“ sind:

Nicht eingeloggter User

Sieht nur eine read‑only Lobby‑Liste (ohne Beträge oder mit Maskierung), Buttons sind disabled, Hinweis „Verbinde Wallet, um beizutreten oder zu erstellen“.
​

Eingeloggter User ohne FACEIT‑Verknüpfung

Kann Lobbys sehen, aber beim Erstellen/Beitreten kommt zuerst der FACEIT‑Link‑Flow (den du schon hast).
​

Eingeloggter & FACEIT‑verknüpfter User

Voller Zugriff: neue Challenge, Beitreten, Ansicht der eigenen Lobbys und History.

Zusätzlich:

Match‑Detail‑Ansicht (/match/:id oder Modal):

Zeigt für beide Spieler alle relevanten Infos: Status, Stake, Game, Modus, Contract‑Adresse, on‑chain‑Status, FACEIT Match‑Link, Oracle‑Bestätigung (z. B. „verifiziert durch 3 Oracles“).
​

1. Empfohlene Umsetzungsreihenfolge (praktischer Ablaufplan)
Backend‑Grundlagen

Tabellen users, matches, match_results anlegen.

kdapp‑Episode‑Typ für Match implementieren (Initialize/Create/Execute‑States).
​

API „happy path“

POST /api/challenges, GET /api/lobbies, POST /api/challenges/{id}/join minimal implementieren (zunächst ohne echte on‑chain‑Calls, nur Dummy‑Status).
​

Frontend-Routing & Screens

/lobby mit statischen Daten aufsetzen, Modal/Seite für „Challenge erstellen“, grundlegender „Beitreten“‑Flow (Mock‑Backend).
​

On-Chain-Anbindung MatchEscrow

createMatch / acceptMatch in Backend einbinden, Events verarbeiten, Status in DB/kdapp updaten.
​

Frontend: Wallet‑Signatur‑Flow einsetzen (aus deiner bestehenden KAS‑Integration).

FACEIT-Match-Erstellung und Oracle-Prototyp

Beim Übergang zu LOCKED → FACEIT‑Match anlegen, external_match_id speichern.
​

Oracle‑Test: manuell oder mit einfachen Scripts resolveMatch aufrufen und End‑to‑End prüfen.

Lobby-Real-Time & UX-Finish

WebSocket/Event‑Stream für Lobby‑Updates.

Loading‑States, Fehler‑Handling (z. B. wenn Deposit fehlschlägt, TIMEOUT‑Fälle).

Verlauf-Basis

GET /api/history + einfache /history‑Seite, die alle RESOLVED‑Matches des Users zeigt.
