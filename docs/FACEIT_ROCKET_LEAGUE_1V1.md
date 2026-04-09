# KaspaBattle2: FACEIT Rocket League 1v1 Integrationsplan

## Kontext

Dieses Dokument beschreibt, wie ein zusätzliches FACEIT-Spiel im 1v1-Modus in KaspaBattle2 integriert werden soll. Der Plan orientiert sich strikt an der bestehenden Counter-Strike‑2‑Integration und nutzt die vorhandene Architektur (battle-api, battle-core, battle-kaspa, battle-frontend).

Allgemeine Annahmen:
- FACEIT stellt Match-Historie, Match-Details und Spieler-Statistiken über die FACEIT Data API zur Verfügung.
- Für das Spiel existiert (oder wird angelegt) ein FACEIT Club/Queue, der 1‑gegen‑1‑Matches mit genau zwei Fraktionen (faction1/faction2) und je genau einem Spieler pro Team erzeugt.
- Die KaspaBattle-Domäne bleibt generell spielunabhängig; der GameType wird nur zur Semantik und UI-Steuerung verwendet.

## FACEIT-spezifische Spielannahmen

- Rocket League besitzt einen nativen 1v1-Modus; FACEIT wird als externe Matchmaking-/Turnier-Plattform dafür genutzt.
- Für KaspaBattle wird ein Rocket‑League‑Club mit 1v1‑Queue benötigt (Duels 1v1, Standard-Regeln).
- Die FACEIT Data API stellt Match- und Spieler-Daten für Rocket League bereit, inklusive Teams/Factions mit genau einem Spieler.

## Backend: battle-core

1. **GameType erweitern (falls noch nicht vorhanden)**
   - Prüfe `kaspabattle/battle-core/src/types.rs` auf vorhandenen `GameType::RocketLeague` Eintrag.
   - Falls nicht vorhanden: Variante hinzufügen, z.B.
     - Enum-Variante `GameType::RocketLeague` mit `#[serde(rename = "rocket_league")]`.
     - `as_str()`-Match-Arm auf `"rocket_league"`.
     - `FromStr`-Arm für `"rocket_league"`.
   - Stelle sicher, dass alle Stellen, die `GameType` matchen, einen Fallback-Arm (`_ =>`) haben oder um die neue Variante ergänzt werden.

2. **Domänenlogik prüfen**
   - Bestätige, dass alle Match-Lifecycle-Operationen (Creation, Lock, Settlement) spielagnostisch sind und nur auf `GameType.as_str()` bzw. der game_id arbeiten.
   - Es dürfen **keine CS2-spezifischen Annahmen** (z.B. Rundenzahl, Map-Typ) in der core-Logik kodiert sein; solche Details gehören in die FACEIT-Konfiguration und nicht in KaspaBattle.

## Backend: battle-api (HTTP /faceit-Endpoints)

1. **ALLOWED_GAME_IDS aktualisieren**
   - Datei: `kaspabattle/battle-api/src/api/faceit.rs`.
   - Stelle sicher, dass `ALLOWED_GAME_IDS` den Eintrag `"rocket_league"` enthält.
   - Dies schützt gegen Path-Injection und stellt sicher, dass nur explizit erlaubte Spiele abgefragt werden.

2. **/faceit/stats & /faceit/matches nutzen game-Parameter**
   - Beide Endpoints akzeptieren `?game=...` und rufen `validate_game_id` auf.
   - Für das neue Spiel ist **keine Codeänderung** nötig, solange `ALLOWED_GAME_IDS` und die FACEIT Data API den game_id unterstützen.
   - Testfälle ergänzen, die `game=rocket_league` verwenden und z.B. über Wiremock/Dummy-Daten Antworten simulieren.

3. **Profil-Endpunkt /faceit/profile**
   - Aktuell wird im Erfolgsfall der ELO/Skill-Level explizit aus `profile.games.get("cs2")` extrahiert.
   - Erweiterung: Für UI-Anzeigen pro Spiel kann optional eine Auswahl-Logik eingeführt werden (z.B. Parameter `game`), ist für das reine Wager-Matching aber nicht zwingend erforderlich.

## Backend: battle-kaspa (Oracle / FACEIT-Client)

1. **FaceitApiClient wiederverwenden**
   - Datei: `kaspabattle/battle-kaspa/src/faceit_api.rs`.
   - Der Client ruft `GET /matches/{match_id}` auf und deserialisiert in `FaceitMatchDetails`.
   - Logik in `verify_players_in_match` und `determine_winner_guid` ist **spielagnostisch** und arbeitet nur mit Factions und Player-IDs.
   - Hier ist **keine spielbezogene Anpassung** nötig, solange das FACEIT-Match ein 1v1 mit genau zwei Fraktionen ist.

2. **OracleService-Integration**
   - Prüfe `kaspabattle/battle-kaspa/src/oracle.rs`, wie Match-IDs, Player-Guids und GameType kombiniert werden.
   - Stelle sicher, dass GameType nur für Logging/Validierung genutzt wird, nicht für CS2-spezifische Regeln.

3. **Tests erweitern**
   - Bestehende Tests in `faceit_api.rs` nutzen ein CS2-Beispiel.
   - Ergänze zusätzliche Testfälle mit `FaceitMatchDetails` für Rocket League (nur `game_id` und ggf. Namensfelder ändern), um zu zeigen, dass die winner-/Verify-Logik unabhängig vom Spiel funktioniert.

## Frontend: battle-frontend

1. **SUPPORTED_GAMES erweitern**
   - Datei: `battle-frontend/src/config/constants.ts`.
   - Füge einen Eintrag in SUPPORTED_GAMES hinzu mit:
     - id = 'rocket_league'
     - name = 'Rocket League'
     - icon = '/rocket_league_logo.svg'
     - platform = 'FACEIT'
   - Lege ein passendes Icon unter `battle-frontend/public/rocket_league_logo.svg` ab.

2. **GameId-Typ & UI-Selektor**
   - `export type GameId = typeof SUPPORTED_GAMES[number]['id'];` funktioniert automatisch nach Hinzufügen.
   - Stelle sicher, dass alle Komponenten, die einen Game-Selektor anzeigen (z.B. Lobby-Erstellung), dynamisch über `SUPPORTED_GAMES` iterieren und **nicht hart auf 'cs2'** codiert sind.

3. **FACEIT-Statistik- und Match-Views**
   - `faceitApi.getStats(game?: string)` und `faceitApi.getMatches(game?: string, ...)` akzeptieren bereits einen `game`-Parameter (Standard: 'cs2').
   - Ergänze UI-Steuerung, so dass beim Wechsel des Spiels der entsprechende game_id an diese Methoden übergeben wird.
   - Optional: Pro Spiel unterschiedliche Statistik-Views (z.B. K/D vs. andere Metriken) konfigurieren; initial kann die generische Darstellung aus CS2 wiederverwendet werden.

## Domain-/API-Modelle

1. **Match-Entitäten**
   - Prüfe, ob im Datenbank-Schema ein Feld `game_type` oder `game_id` existiert und ob der Wert beim Anlegen eines Matches gesetzt wird.
   - Stelle sicher, dass das UI beim Erstellen eines Lobbys den gewählten `GameId` an die API übergibt und diese ihn in `GameType` umwandelt.

2. **Validierung 1v1-Invariante**
   - Ergänze (falls noch nicht vorhanden) eine Validierung im Oracle-Workflow, die sicherstellt:
     - `teams.faction1.roster.len() == 1` und `teams.faction2.roster.len() == 1`.
     - Die in der Wager-Challenge hinterlegten FACEIT-Player-Guids exakt mit den beiden Spielern übereinstimmen.
   - Bei Verstoß: Match als `DISPUTED` markieren oder Fehlermeldung/logische Exception auslösen, statt automatisch auszuzahlen.

## Manuelle FACEIT-Konfiguration

1. **Club/Queue anlegen**
   - Erstelle in FACEIT einen Club für Rocket League und füge eine Queue hinzu, die:
     - Rocket League als Spiel nutzt.
     - 1 Spieler pro Team (1v1) erlaubt.
     - Die gewünschten Regionen/Server-Standorte konfiguriert.

2. **Regeln dokumentieren**
   - Definiere in den Club-Regeln klar, welche Win-Condition für KaspaBattle zählt (Standard-Match-Win, First Blood, etc.).
   - Stelle sicher, dass diese Win-Condition mit den von der Data API gelieferten Resultaten konsistent ist (z.B. nur vollständige Match-Siege verwenden).

3. **Integration testen**
   - Manuell ein Test-Match zwischen zwei Test-Accounts in der 1v1-Queue spielen.
   - Über das KaspaBattle-Frontend ein entsprechendes Wager-Match anlegen, Deposits tätigen und die automatische Auswertung durchlaufen lassen.
   - Logs auf eventuelle game_id-/Schema-Probleme prüfen.

## Rollout-Checkliste

- [ ] `GameType` enthält rocket_league-Variante und `FromStr`/`as_str()` sind aktualisiert.
- [ ] `ALLOWED_GAME_IDS` in `battle-api` enthält "rocket_league".
- [ ] Frontend `SUPPORTED_GAMES` enthält rocket_league inkl. Icon.
- [ ] Alle relevanten Komponenten nutzen dynamische `GameId`-Auswahl, keine Hard-Codes auf 'cs2'.
- [ ] Oracle-/FACEIT-Integration wurde mit mindestens einem realen Match getestet.
- [ ] Dokumentation/FAQ zur Nutzung von Rocket League in KaspaBattle wurde aktualisiert.
