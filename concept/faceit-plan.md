# Projekt- und Ablaufplan: FACEIT-Integration für KaspaBattle

## 0. Zielbild

Funktionen:

1. **„Login with FACEIT“ Button**
   - User klickt Button auf deiner Seite
   - Wird zu FACEIT geleitet, loggt sich ein, gibt Consent
   - Kommt zu deiner App zurück, ist eingeloggt (Session/JWT)
   - Du speicherst: `faceit_guid`, Nickname, Avatar, E‑Mail

2. **Spieldaten & Stats abrufen**
   - Du kennst die `player_id`/`guid` des Users
   - Über die **FACEIT Data API** (REST, JSON) kannst du u. a.:
     - Player‑Details holen: `/players/{player_id}`
     - Match‑History: `/players/{player_id}/history`
     - Stats für ein Game (z.B. CS2): `/players/{player_id}/stats/{game_id}` oder `/players/{player_id}/games/{game_id}/stats`
   - Authentifizierung für Data API über `Authorization: Bearer <api_key>`

3. **Später (für KaspaBattle)**
   - Match‑Ergebnisse als „Oracle‑Daten“ für deine Wetten verwenden
   - ELO/Stats für Matchmaking, Limits, etc.

---

## 1. Architektur & Technologie-Entscheidungen

- **Frontend**:
  - Web (z.B. React/Next.js) mit Button „Login with FACEIT“
- **Backend**:
  - Beliebig (Node/Express, NestJS, Spring Boot, etc.)
  - Backend ist OAuth‑Client, speichert Secrets & macht Server‑zu‑Server Calls
- **Auth‑Flow**:
  - FACEIT Connect mit **OAuth2 Authorization Code Flow mit PKCE**
- **Datenschicht**:
  - DB‑Tabellen für `users`, `faceit_accounts`, `matches`, `player_stats`

Optional: Nutzung von Libraries wie **Auth.js / NextAuth FACEIT Provider** oder **Passport.js Strategy `passport-faceit`**.

---

## 2. Setup in der FACEIT Developer Console

**Ziel:** Du hast alle Keys & IDs, die du für Login und Data API brauchst.

1. **FACEIT Developer Konto & App anlegen**
   - Auf developer.faceit.com einloggen
   - Neues Projekt / neue App in der App‑Sektion anlegen

2. **FACEIT Connect (OAuth2) konfigurieren**
   - OAuth2 Client / Consent Screen erstellen:
     - Icon/Name deiner App
     - **Redirect URL**: z.B. `https://deine-domain.com/auth/faceit/callback` (für Dev: `http://localhost:3000/api/auth/faceit/callback` o.ä.)
   - Wichtige Daten merken:
     - `CLIENT_ID`
     - `CLIENT_SECRET`
     - `REDIRECT_URI`

3. **Scopes definieren**
   - Für Login / Identity typischerweise:
     - `openid`, `email`, `profile`
   - Damit bekommst du:
     - GUID (eindeutige FACEIT User-ID)
     - Nickname
     - Avatar
     - (je nach Konfiguration) E‑Mail

4. **Data API Key anlegen**
   - In der Developer Console einen **Data API Key** erstellen (Server‑Side Key)
   - Notieren:
     - `FACEIT_DATA_API_KEY`
   - Base URL der Data API: `https://open.faceit.com/data/v4`

---

## 3. Login mit FACEIT (OAuth2 Flow)

### 3.1. Ablauf

1. **Route für Login-Start**
   - Backend‑Route: `GET /auth/faceit`
   - Aufgabe:
     - `state` (CSRF‑Schutz) generieren und speichern (z.B. in Session)
     - PKCE Code Challenge/Verifier generieren (wenn du PKCE nutzt)
     - User per `302` zu FACEIT OAuth2 Authorization Endpoint umleiten mit Parametern:
       - `client_id`
       - `redirect_uri`
       - `response_type=code`
       - `scope=openid email profile`
       - `state=<random>`
       - `code_challenge` & `code_challenge_method` (für PKCE)

2. **FACEIT Login & Consent**
   - User loggt sich auf FACEIT ein und gibt deiner App Zugriff

3. **Callback-Route**
   - Backend‑Route: `GET /auth/faceit/callback`
   - FACEIT ruft diese an mit:
     - `code=<authorization_code>`
     - `state=<dein_state>`
   - Implementieren:
     - `state` gegen gespeicherten Wert prüfen (CSRF)
     - Fehlerfälle (fehlerhafter/fehlender Code, abgebrochener Login) behandeln

4. **Code gegen Tokens tauschen**
   - Dein Backend macht Server‑zu‑Server `POST` an das Token‑Endpoint:
     - Header:
       - `Authorization: Basic base64(CLIENT_ID:CLIENT_SECRET)`
       - `Content-Type: application/x-www-form-urlencoded`
     - Body:
       - `grant_type=authorization_code`
       - `code=<authorization_code>`
       - `redirect_uri=<deine_redirect_uri>`
   - Antwort enthält:
     - `access_token`
     - `id_token` (JWT mit User‑Infos)
     - `refresh_token`

5. **ID Token validieren & User anlegen**
   - `id_token` mit JWT‑Lib validieren (Signatur, Issuer, Audience, Expiry)
   - Relevante Felder z.B.:
     - `guid` (FACEIT User-ID)
     - `nickname`
     - `picture` (Avatar)
     - `email`
   - In deiner DB:
     - Wenn `guid` schon existiert → User laden
     - Sonst neuen User & Faceit‑Account anlegen
   - Server‑seitig Session/JWT setzen (z.B. `Set-Cookie`), so ist der User in deiner App eingeloggt

6. **Frontend-Integration des Buttons**
   - Button „Login with FACEIT“
   - Klick → Redirect auf `/auth/faceit`
   - Nach Login:
     - User auf Dashboard/Profilseite leiten
     - Nickname & Avatar anzeigen

### 3.2. Alternative: Nutzung fertiger Libraries

- **NextAuth / Auth.js FACEIT Provider**
  - Provider mit `clientId`, `clientSecret` konfigurieren
  - Callback URL: `/api/auth/callback/faceit`
  - Token‑Management, Sessions, CSRF, PKCE etc. werden von der Library übernommen

- **Passport-FACEIT** (Node/Express)
  - `passport.use(new FaceitStrategy({ clientID, clientSecret }, verifyCallback))`
  - Routen:
    - `/auth/faceit` (Start)
    - `/auth/faceit/callback` (Callback)
  - Im `verify`‑Callback:
    - `accessToken`, `refreshToken`, `params`, `profile`/`userData` auslesen
    - User in DB anlegen / updaten

---

## 4. Spiele & Stats des Users abrufen (Data API)

Sobald du die FACEIT `guid`/`player_id` kennst, kannst du über die Data API alle relevanten Infos holen.

### 4.1. Data API Client-Service

Backend‑Service „FaceitDataService“:

- Konfiguration:
  - `BASE_URL = https://open.faceit.com/data/v4`
  - Header: `Authorization: Bearer ${FACEIT_DATA_API_KEY}`
- Wichtige Funktionen:

1. `getPlayerById(player_id)`
   - Endpoint: `GET /players/{player_id}`
   - Liefert u. a.:
     - Nickname, Avatar, Games, Skill Level, Region

2. `getPlayerHistory(player_id, game_id, offset, limit)`
   - Endpoint: `GET /players/{player_id}/history?game={game_id}&offset=&limit=`
   - Liefert:
     - Liste der Matches (Match IDs, Ergebnisse, Maps, Timestamps)

3. `getPlayerStats(player_id, game_id)`
   - Endpoint (je nach finalem Spec):
     - `GET /players/{player_id}/stats/{game_id}` oder
     - `GET /players/{player_id}/games/{game_id}/stats`
   - Liefert:
     - K/D, Winrate, Anzahl Matches, Serien, Headshot‑Rate, etc.

4. Optional: `getMatchDetails(match_id)`
   - Für spätere Wettauswertung relevant (Score, Teams, Maps, Ergebnis)

### 4.2. Anwendungsfälle in deiner App

1. **User-Profilseite**
   - Beim Aufruf:
     - `getPlayerById(faceit_guid)` laden
     - `getPlayerStats(faceit_guid, 'cs2')` (oder gewünschtes Game)
   - Anzeigen:
     - Rank / ELO / Level
     - Lifetime‑Stats

2. **„Meine letzten Spiele“**
   - API‑Route: `GET /me/faceit/matches`
   - Backend kennt `user_id` → `faceit_guid`
   - Ruft `getPlayerHistory(faceit_guid, 'cs2', ...)` auf
   - Ergebnis in UI paginiert anzeigen

3. **Vorbereitung für KaspaBattle‑Wetten**
   - Definieren, welche Match‑Daten du zur Validierung eines Wetteinsatzes brauchst
   - Beim Wettausgang:
     - Mit `match_id` an Data API → Matchdetails holen
     - Prüfen, ob das Ergebnis wie erwartet ist
     - Diese Info später als Input für deine Kaspa‑On‑Chain‑Logik verwenden

---

## 5. Interne Datenmodelle & Caching

Ziele:
- Minimale Calls an FACEIT API (Rate Limits, Latenz)
- Historie und Snapshots für Auswertungen

### 5.1. Datenmodelle

1. **Users**
   - `id` (intern)
   - `email` (optional)
   - `created_at`, `updated_at`

2. **FaceitAccounts**
   - `user_id` (FK)
   - `faceit_guid`
   - `nickname`
   - `avatar_url`
   - `linked_at`

3. **FaceitStatsSnapshots**
   - `faceit_guid`
   - `game_id` (z.B. `cs2`)
   - `snapshot_at`
   - `elo`, `rank`, `winrate`, `matches_played`, etc.

4. **FaceitMatches**
   - `match_id`
   - `faceit_guid`
   - `game_id`
   - `started_at`, `finished_at`
   - `map`
   - `score_team1`, `score_team2`
   - `result` (win/lose)

### 5.2. Caching-Strategien

- Beim Aufruf „Meine letzten Spiele“:
  - Wenn Daten älter als X Minuten → frische Daten von FACEIT holen + lokal updaten
  - Sonst: aus DB liefern
- Hintergrundjobs:
  - Optional Cron‑Job, der für aktive User periodisch Stats aktualisiert

---

## 6. Sicherheit, Fehlerbehandlung & UX

1. **Security**
   - FACEIT `CLIENT_SECRET` und `DATA_API_KEY` nur im Backend speichern
   - HTTPS erzwingen (Login & Callback)
   - `state` und PKCE zum Schutz vor CSRF & Code‑Injection nutzen
   - ID‑Token Signatur prüfen (Issuer, Audience, Expiry)

2. **Fehlerfälle abfangen**
   - User bricht Login bei FACEIT ab → verständliche Fehlermeldung anzeigen
   - 4xx/5xx von Data API:
     - Rate Limit Handling (Retry mit Backoff, Meldung an User)
     - Fallback: „FACEIT derzeit nicht erreichbar“

3. **UX**
   - Loading‑Zustände für Login & Datennachladen
   - Klare Hinweise, wenn FACEIT Account nicht verlinkt ist
   - Option „Re‑Link FACEIT Account“

---

## 7. Konkrete Milestones / Abend-Plan

### 7.1. Session 1: FACEIT Login (Ende‑zu‑Ende)

1. Developer‑Portal Setup fertig machen:
   - OAuth2 Client + Redirect URL + Scopes
   - Data API Key notieren (muss in dieser Session noch nicht genutzt werden)

2. Backend‑Routes bauen:
   - `GET /auth/faceit` → Redirect zu FACEIT
   - `GET /auth/faceit/callback` → Code empfangen, gegen Tokens tauschen, User in DB anlegen

3. Frontend:
   - Einfacher Button „Login with FACEIT“ → Redirect auf `/auth/faceit`

4. Test:
   - Kompletten Flow einmal durchspielen, in DB prüfen, ob der FACEIT GUID & Nickname gespeichert wurden

### 7.2. Session 2: Spiele & Stats nachladen

1. Data API Client implementieren:
   - HTTP‑Client mit `Authorization: Bearer <DATA_API_KEY>`
   - Methoden: `getPlayerById`, `getPlayerHistory`, `getPlayerStats`

2. Routen:
   - `GET /me/faceit/profile`
   - `GET /me/faceit/matches`

3. UI:
   - Auf Profilseite Basic‑Stats anzeigen
   - Liste der letzten Matches anzeigen
