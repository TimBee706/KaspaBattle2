# KaspaBattle TournamentUpdate – Implementierungsplan

## 0. Ziel & Scope

Dieser Plan beschreibt alle Schritte, die nötig sind, um die Turnier-Funktionalität konsistent zur Lobby-Experience zu gestalten, die bekannten Fehler zu beheben und die Sichtbarkeit des Tournament-Features korrekt an den Testmodus zu koppeln.

Am Ende soll ein Coding Agent anhand dieses Dokuments alle beschriebenen Anpassungen in Frontend und Backend umsetzen können.

---

## 1. UI-Angleichung: Turnier-Übersicht & -Erstellung

### 1.1 Betroffene Komponenten/Dateien

- **Turnier-Liste & Erstellung**  
  - `battle-frontend/src/pages/TournamentListPage.tsx`  
  - Inline-Komponente `CreateTournamentModal` in derselben Datei
- **Turnier-Detailseite**  
  - `battle-frontend/src/pages/TournamentDetailPage.tsx`
- **Referenz-Lobby-Ansichten (für Look & Feel)**  
  - `battle-frontend/src/pages/LobbyPage.tsx` (Übersicht & CTA-Button)  
  - `battle-frontend/src/pages/CreateMatchPage.tsx` + `components/match/CreateChallengeForm` (Erstell-Flow)

### 1.2 Designprinzipien, an denen sich Turnier-UI orientieren soll

- **Keine Emojis in Turnier-UI**
  - Entfernen von Emojis in Turnier-spezifischen Überschriften, Buttons, Badges und Status-Labels (z.B. "🏆", "🔒", "⚖️", etc.).
  - Statt Emojis: klare, textbasierte Labels oder neutrale Icons (falls vorhanden) im konsistenten Designsystem.

- **Typografie & Größen**
  - Hauptüberschriften analog zur Lobby:
    - Lobby: `text-4xl font-black` im Header.
    - Turnierliste: aktuell `text-3xl font-black` – ggf. auf 4xl hochziehen oder bewusst als Sekundär-Feature belassen; dies im Design einmal konsistent entscheiden.
  - Buttons: gleiche Größen und Kapitalisierung wie in Lobby (`font-black`, `uppercase` bei Primäraktionen wie „Create Challenge“ vs. „Turnier erstellen“).

- **Buttons & Farben**
  - Primär-Buttons für zentrale Aktionen ("Turnier erstellen", "Team anmelden", "Bracket sperren") sollen die gleiche visuelle Gewichtung haben wie Lobby-CTA (`bg-kaspa-primary`, `text-kaspa-dark`, starke Schatten, abgerundete Ecken).
  - Sekundär- und Danger-Actions (Abbrechen, Disput, Cancel Tournament) mit konsistenten Border-/Farb-Patterns (z.B. `bg-white/5` + `text-gray-400` für neutral, `bg-red-600/20` für destructive).

- **Layout & Informationshierarchie**
  - Tournament-Card (Liste): gleiche logische Reihenfolge von Infos wie Lobby-Tabellenzeilen: Name → Einsatz/Stake → Spieler/Teams → Status.
  - Detail-Header: links Title + Meta-Info (Game, Teamcount, Status), rechts Actions, ähnlich wie die Header-Layouts in Match-/Lobby-Ansichten.

### 1.3 Konkrete Anpassungen Turnier-Liste & -Erstellung

**1.3.1 `TournamentListPage` – Header & Layout angleichen**

- Header-Container auf das gleiche Layout-Pattern wie `LobbyPage` bringen:
  - Außen-Container von `min-h-screen bg-[#070d14]` auf `container mx-auto px-4 py-8` oder ein konsistentes Layout mit den übrigen Seiten vereinheitlichen.
  - Überschrift-Block: Title + Subtitle analog zur Lobby (uppercase, Tracking, Secondary Label).
- Filter-Buttons visuell näher an Lobby-Buttons bringen:
  - Einheitliche Border-Radien/Fonts, ggf. kein Emoji im "Live" / "Beendet" Label.

**1.3.2 `CreateTournamentModal` – Formular an CreateChallengeForm anpassen**

- Labels und Inputs:
  - Gleiche Input-Styles wie im Match-Create-Form:
    - z.B. konsistent `bg-kaspa-card`/`bg-[#1a2332]`, Border-Farben, Focus-States.
  - Platzierung: zweispaltiges Grid nur, wenn es auch im Lobby-Form so gemacht wird; sonst vereinheitlichen (z.B. einspaltig auf Mobile, klarer Stack).
- Buttons:
  - Submit-Button mit denselben Utility-Klassen wie Lobby-Primär-Button (Farbpalette `bg-kaspa-primary`, `text-kaspa-dark`, starke Shadow) und ohne Emoji im Label.
  - Close-Icon/Schließen-Button: optional durch textbasierten "Abbrechen"-Button ergänzen oder konsistent zum restlichen Design halten.
- Hilfstexte:
  - Unterhalb des Titels optional einen kurzen Hinweis einbauen, dass Turniere aktuell nur für CS2 verfügbar sind (siehe Abschnitt 3), damit die Einschränkung sofort sichtbar ist.

### 1.4 Konkrete Anpassungen Turnier-Detailseite

**1.4.1 Header-Bereich (`TournamentDetailPage`)**

- Emojis in Action-Buttons entfernen:
  - `Ergebnisse`, `Bracket sperren`, `Disput`, `Abbrechen` ohne vorangestelltes Emoji.
- Status-Badge: Farben und Größe an andere Status-Badges (z.B. Lobby/Match) angleichen (Abgleich mit `StatusBadge` Implementierungen).
- Meta-Zeile unter dem Titel:
  - Reihenfolge der Infos fest definieren: Game → Teams (x / max) → Deadline.
  - Schriftgrößen und Farben: an Lobby-Unterzeilen anlehnen (z.B. `text-slate-500 text-sm font-bold uppercase tracking-widest`).

**1.4.2 Tabs & Cards**

- Tabs (`overview`, `teams`, `bracket`) so stylen, dass sie sich wie die restlichen Navigations-Tabs der App anfühlen (Border-Radius, aktiver Hintergrund, Schriftgröße).
- Infos im Overview-Tab als Definition-List beibehalten, aber Abstände und Fonts konsistent zu anderen Info-Karten setzen.
- Prize-Pool-Banner: Emojis aus Labels (`🥇`, `🥈`, `🏛️`) entfernen und ggf. durch neutrale Icons oder reine Textlabels ersetzen.

**1.4.3 Teams-Tab**

- Teamkarten-Layout an Lobby-Tabellen anlehnen:
  - Name links, Status (funded/awaiting) rechts, keine Emojis.
  - Eigene Teams klar, aber nüchtern hervorheben (Badge ohne Emoji).

**1.4.4 Admin-Ansichten (falls in Frontend sichtbar)**

- In `TournamentAdminPage` Emojis in Status-/Disput-Titeln entfernen und Layout der Cards an Turnier-Detailseite angleichen.

**1.4.5 Übersetzungen bereinigen**

- In `battle-frontend/src/locales/de/common.json` und `en/common.json` alle Turnier-bezogenen Keys von Emojis säubern:
  - `tournaments.filter.live`, `tournaments.filter.completed`, `tournaments.filter.disputed`, `tournaments.filter.cancelled` etc.  
  - `tournaments.detail.actions.*`, `tournaments.detail.prize_banner.*`, `tournaments.admin.*`, `tournaments.results.placements.*`.
- Dabei die Keys nicht umbenennen, nur die Werte anpassen, um keine Codeänderungen an den i18n-Keys zu erzwingen.

---

## 2. Fehler bei Team-Registrierung (409 Conflict)

### 2.1 Kontext & betroffene Stellen

- Frontend:
  - Team-Registrierung läuft über `registerTeam` in `battle-frontend/src/api/tournaments.ts` (nicht explizit hier aufgeführt) und wird in `TournamentDetailPage` im Teams-Tab genutzt.
  - Fehlermeldung im Browser: `POST /api/v1/tournaments/:id/teams 409 (Conflict)`.
- Backend:
  - Endpoint `POST /api/v1/tournaments/:id/teams` implementiert in `kaspabattle/battle-api/src/api/tournament.rs` in `register_team`.

### 2.2 Mögliche Ursachen laut Backend-Logik

In `register_team` gibt es drei Stellen, an denen ein 409 zurückgegeben wird:

1. **Turnier nicht mehr im Status REGISTRATION**
   - Wenn `status != "REGISTRATION"` → Fehler `registration_closed` (409).
2. **Anmeldeschluss überschritten**
   - Wenn `registration_deadline` gesetzt ist und `now > deadline` → Fehler `registration_closed` (409).
3. **Turnier voll**
   - Wenn `COUNT(*) FROM tournament_teams >= max_teams` → Fehler `tournament_full` (409).
4. **Teamname bereits vergeben (Unique-Constraint)**
   - Beim Insert in `tournament_teams`: bei Unique-Verletzung → Fehler `team_name_taken` (409).

### 2.3 Geplanter Debug- & Fix-Ansatz

**2.3.1 Fehlermeldungen im Frontend korrekt anzeigen**

- In `TournamentDetailPage` wird im `catch` Block von `handleRegister` bereits versucht, `err.response.data.message` zu lesen.  
  Plan:
  - Sicherstellen, dass alle oben genannten Fehler im Backend ein aussagekräftiges `message`-Feld setzen (ist in `tournament.rs` bereits der Fall).  
  - Frontend: Falls `message` fehlt, einen generischen, lokalisierten Fallback verwenden (z.B. `tournaments.detail.teams.register_error_generic`).
  - Optional: Mapping `error`-Code → spezifische, lokalisierte Messages, um präzisere Meldungen zu zeigen (z.B. eigener Text für "Turnier voll", "Deadline überschritten" etc.).

**2.3.2 UX-Verbesserungen zur Vermeidung von 409**

- Team-Registrierungs-Button auf Detailseite nur anzeigen, wenn
  - `tournament.status === 'REGISTRATION'`,
  - `teams.length < tournament.max_teams`,
  - `registration_deadline` noch nicht überschritten (Client-seitig vorprüfen, rein als UX-Hilfe).
- Bereits angemeldeter Captain:
  - Falls der eingeloggte User schon Captain eines Teams in diesem Turnier ist (`myTeamIds.length > 0`), den "Team anmelden" Button komplett ausblenden (ist im Code bereits so angelegt, prüfen und ggf. schärfen).

**2.3.3 Technischer Debug für das aktuelle 409-Problem**

- Im Coding Agent Schritt:
  - Konsolen-Logging temporär erweitern, um `error.response.data` vollständig zu inspizieren (inkl. `error`-Code).  
  - Testfälle durchspielen:
    - Neues Turnier im Status `REGISTRATION`, kein Deadline, leere Teams → Registrierung sollte 201 liefern.
    - Turnier auf Status `REGISTRATION` + `max_teams` erreichen → erwarteter 409 `tournament_full`.
    - Deadline in Vergangenheit setzen und registrieren → erwarteter 409 `registration_closed`.
  - Falls der vorliegende 409 keiner der obigen Konstellationen entspricht (z.B. DB-Constraint), Ursache in DB-Schema suchen und gesondert dokumentieren.

---

## 3. Klarstellen: Turniere aktuell nur für CS2

### 3.1 Backend-Status

- Backend `CreateTournamentReq` verwendet das Feld `game_id` als `game_type` mit Default `"CS2"` und speichert diesen Wert in der DB.
- Praktisch unterstützt der Turnier-Flow damit aktuell nur CS2.

### 3.2 Frontend – Turniererstellung

**3.2.1 Explizite UI-Kommunikation in `CreateTournamentModal`**

- Derzeit ist `game_id` hart auf `cs2` gesetzt und es gibt kein UI-Feld für die Spielauswahl.
- Anpassungen:
  - Unter dem Turniernamen oder über dem Formular einen deutlichen Hinweis einfügen:  
    - z.B. "Aktuell sind Turniere nur für Counter-Strike 2 (CS2) verfügbar."  
    - Übersetzungen für DE/EN in `tournaments.create.game_hint` o.ä. ergänzen.
  - Optional: kleines, disabled Select-Feld für `game_id` mit nur einer Option "CS2" anzeigen, um künftige Mehrspielersupport vorzubereiten, aber visuell klarzumachen, dass andere Spiele noch nicht auswählbar sind.

**3.2.2 Turnierliste & Details**

- In Karten (`TournamentCard`) wird `tournament.game_id` angezeigt.  
  - Sicherstellen, dass der Wert konsistent z.B. als `CS2` gerendert wird (Uppercase) und ggf. mit Label "Spiel: CS2" versehen ist.
- Auf der Detailseite im Overview-Tab den Spieleintrag klar benennen, z.B. "Spiel: CS2 (Counter-Strike 2)".

**3.2.3 Validation/Sicherheit (optional)**

- Wenn Frontend in Zukunft ein Eingabefeld für `game_id` bekommt, zusätzlich Client-seitig nur `cs2` erlauben, solange Backend nur dieses Spiel unterstützt.

---

## 4. Tournament Mode Callout nur im Testmodus anzeigen

### 4.1 Betroffene Stellen

- Hauptseite / Landing Page: `battle-frontend/src/pages/LandingPage.tsx`.
- Callout-Sektion ganz unten: "Tournament Mode Feature Callout" mit Text "🏆 New — Tournament Mode" und CTA "Browse Tournaments".
- Testmodus-Flag: `testMode` in `useAuthStore` (`battle-frontend/src/stores/useAuthStore.ts`), aktuell u.a. in `LobbyPage` gesetzt, wenn `display_name === "TestUser"`.

### 4.2 Implementierungsplan

1. **TestMode in LandingPage aus Store lesen**
   - In `LandingPage` zusätzlich zum bisherigen Destructuring aus `useAuthStore` auch `testMode` beziehen:
     - `const { isAuthenticated, isFullyConnected, testMode } = useAuthStore();`

2. **Tournament-Callout konditional rendern**
   - Die gesamte Sektion
     ```tsx
     {/* Tournament Mode Feature Callout */}
     <section className="py-16 ...">
       ...
     </section>
     ```
     in einen Guard kapseln, z.B.:
     ```tsx
     {testMode && (
       <section ...>
         ...
       </section>
     )}
     ```
   - Alternative: Statt komplett auszublenden, bei `!testMode` nur einen sehr dezenten "Coming Soon"-Hint anzeigen – gemäß Anforderung aber aktuell komplett ausblenden.

3. **Konsequenzen für produktive Nutzer**
   - Nur Tester (z.B. `TestUser`) sehen den Tournament Mode Callout auf der Startseite.  
   - Direkte Pfade `/tournaments` bleiben erreichbar, aber nicht prominent beworben.

---

## 5. Aufgabenpakete für den Coding Agent

### Paket A – UI-Konsolidierung Turniere

1. Alle Emojis aus Turnier-bezogenen Komponenten (`TournamentListPage`, `TournamentDetailPage`, `TournamentAdminPage`, `TournamentResultsPage`) entfernen oder durch neutrale Icons/Text ersetzen.
2. Layout, Typografie und Button-Styles der Turnierlisten- und -detailansichten an Lobby-/Match-Seiten angleichen (Container, Header, CTA-Buttons).
3. `CreateTournamentModal` optisch an `CreateChallengeForm` orientieren (Form-Fields, Button-Styles, Spacing).
4. Übersetzungsdateien für Turnier-Keys bereinigen (Emojis raus, Texte ggf. schärfer formulieren), ohne Keys umzubenennen.

### Paket B – Team-Registrierungsfehler (409) analysieren & verbessern

1. Backend: Sicherstellen, dass alle 409-Pfade (`registration_closed`, `tournament_full`, `team_name_taken`) aussagekräftige `message`-Felder liefern.
2. Frontend: `TournamentDetailPage` so erweitern, dass diese Messages übersetzt und klar im UI angezeigt werden (unter dem Teamname-Feld).
3. Bedingungen für Anzeige des "Team anmelden"-Buttons schärfen (Status, maxTeams, Deadline, bereits vorhandenes Team des Nutzers).
4. Manuelle Testfälle definieren und durchspielen (inkl. Screenshot/Notizen), um 409-Szenarien zu validieren.

### Paket C – CS2-Only-Kommunikation

1. Hinweistext in `CreateTournamentModal` einfügen und übersetzen („Aktuell nur CS2 Turniere“).
2. Sicherstellen, dass `game_id`/`game_type` konsistent als "CS2" dargestellt wird (Uppercase) und im UI klar als Spieltyp erkennbar ist.
3. (Optional) Disabled Game-Selector vorbereiten, falls Multi-Game-Support geplant ist.

### Paket D – TestMode-gesteuerter Tournament Mode Callout

1. `testMode` in `LandingPage` aus `useAuthStore` holen.
2. Tournament-Callout-Sektion nur rendern, wenn `testMode === true`.
3. Manuell verifizieren:  
   - Mit TestUser (oder manuell gesetztem `setTestMode(true)`) ist der Callout sichtbar.  
   - Für normale Nutzer ist er ausgeblendet.

---

## 6. Abnahme-Kriterien

- **UI**
  - Turnier-Seiten wirken visuell konsistent mit Lobby-/Match-Seiten (kein Mischmasch aus Emojis und unterschiedlichen Button-Stilen).
  - Formular für Turniererstellung ist klar lesbar, mobil nutzbar und an Create-Challenge-Form angelehnt.
- **Funktionalität**
  - Team-Registrierung funktioniert im Normalfall (leeres Turnier, offene Registrierung) ohne Fehler.
  - In allen 409-Fällen erhält der User eine verständliche, spezifische Fehlermeldung (z.B. Turnier voll, Anmeldeschluss vorbei, Name vergeben).
- **Kommunikation**
  - Beim Erstellen eines Turniers ist klar ersichtlich, dass aktuell nur CS2 unterstützt wird.
  - Tournament Mode Callout auf der Landing Page ist ausschließlich im Testmodus sichtbar.

