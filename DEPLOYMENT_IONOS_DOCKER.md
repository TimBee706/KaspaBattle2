# Kaspa Battle — Docker-Deployment auf einem IONOS-Linux-Server (ohne eigene Kaspa-Node)

Diese Anleitung beschreibt das Deployment der konsolidierten Codebasis (`main`, nach Merge von
`TournamentUpdate`) auf einem frisch aufgesetzten IONOS-Linux-Server per Docker Compose. Es wird
**keine eigene Kaspa-Node betrieben** — alle Kaspa-RPC-Aufrufe gehen an einen externen, per
Environment-Variable konfigurierbaren Endpunkt (Resolver-basiert per Default, oder ein expliziter
Provider).

## 1. Voraussetzungen auf dem Server

- Linux-Distribution mit Docker-Support (z. B. Ubuntu 22.04/24.04 LTS — Standard bei IONOS)
- Docker Engine ≥ 24 und Docker Compose Plugin (`docker compose version` sollte funktionieren)
- Offene Firewall-Ports: `80`, `443` (TCP+UDP für HTTP/3) für Caddy; `22` für SSH. **Nicht** öffentlich
  freigeben: `5432` (Postgres, ist bereits per Compose an `127.0.0.1` gebunden), `8080`/`5173`
  (Backend/Frontend — laufen intern im Docker-Netz, öffentlicher Zugriff nur über Caddy/443)
- Eine Domain, die auf die Server-IP zeigt (für Let's-Encrypt-Zertifikate via Caddy)
- Kein `kaspad` nötig — der `kaspa-node`-Service in `docker-compose.yml` bleibt auskommentiert

## 2. Setup-Schritte

```bash
git clone <repo-url> kaspabattle && cd kaspabattle
git checkout main   # enthält nach dem Merge alle Tournament-Features

# Env-Dateien aus dem Template anlegen (siehe .env.docker.example für alle Variablen)
cp .env.docker.example .env.production
cp .env.docker.example .env
# .env.production: von Docker Compose per env_file in den backend-Container geladen.
# .env: von Docker Compose selbst für ${VAR}-Substitution in docker-compose.yml gelesen
#       (POSTGRES_*, DOMAIN, TLS_EMAIL, FRONTEND_BUILD_TARGET, VITE_*).
# Beide Dateien jetzt mit echten Werten befüllen (siehe Abschnitt 4) — niemals committen,
# beide sind bereits in .gitignore.

# Für die Produktions-Domain: Caddy-Config aus dem Template anlegen
cp Caddyfile.example Caddyfile
# In .env: FRONTEND_BUILD_TARGET=prod, DOMAIN=<eure-domain>, TLS_EMAIL=<eure-email> setzen.

docker compose --profile proxy up -d --build
```

Logs/Status:

```bash
docker compose ps
docker compose logs -f backend
docker compose logs -f caddy
```

## 3. Kaspa-RPC-Konfiguration wechseln

Alles läuft über `KASPA_USE_EXPLICIT_NODE` / `KASPA_NODE_URL` / `KASPA_NETWORK` in
`.env.production` — nirgends im Code oder in `docker-compose.yml` ist eine feste Node-Adresse
hinterlegt:

- **Standard (empfohlen zum Start):** `KASPA_USE_EXPLICIT_NODE=false` — `battle-api` nutzt den
  öffentlichen Kaspa-Resolver zur automatischen Node-Auswahl, kein Provider-Vertrag nötig.
- **Fester Provider:** `KASPA_USE_EXPLICIT_NODE=true` und `KASPA_NODE_URL=wss://<provider>:<port>`
  setzen. **`wss://` bevorzugen, wenn der Provider es anbietet** — Klartext-`ws://` ist ein bekannter
  Schwachpunkt der bisherigen Konfiguration (siehe Abschnitt 6).
- Wechsel erfordert nur einen Neustart des `backend`-Containers: `docker compose up -d --build backend`.

## 4. Secrets, die vor dem ersten Start gesetzt werden müssen

In `.env` **und** `.env.production` (siehe `.env.docker.example` für alle Variablen mit Kommentaren):

| Variable | Zweck |
|---|---|
| `POSTGRES_PASSWORD` | Starkes DB-Passwort (Compose bricht ohne Wert hart ab — beabsichtigt) |
| `KASPA_MNEMONIC` / `TREASURY_MNEMONIC` | Frische, nur für diese Umgebung generierte 12-Wort-Mnemonics — niemals wiederverwenden |
| `ORACLE_PRIVATE_KEY` | `openssl rand -hex 32` — Start bricht ohne Wert hart ab (beabsichtigt, fail-closed) |
| `ADMIN_API_KEY`, `ORACLE_API_KEYS`, `FACEIT_WEBHOOK_SECRET` | Je `openssl rand -hex 32` |
| `DOMAIN`, `TLS_EMAIL` | Nur für `--profile proxy` (Caddy) relevant |
| `FRONTEND_URL`, `CORS_ALLOWED_ORIGINS` | Echte Domain statt `localhost` |

Alle Platzhalter im Template sind bewusst offensichtlich ungültig (`CHANGE_ME_*`) — es sind **keine**
Dummy-Mnemonics oder Test-Secrets im Repo in produktiven Pfaden hinterlegt (verifiziert: `.env`,
`.env.production`, `.env.docker` sind alle `.gitignore`t; nur `.env.docker.example` mit reinen
Platzhaltern ist getrackt).

## 5. Lokal testen

Ohne echte Domain/Caddy, nur die drei Kern-Services:

```bash
cp .env.docker.example .env.production
cp .env.docker.example .env
# POSTGRES_PASSWORD, ORACLE_PRIVATE_KEY etc. auch lokal auf echte (Test-)Werte setzen —
# der Start bricht sonst bewusst ab (fail-closed, siehe main.rs).
docker compose up --build
```

- Backend: `http://localhost:8080` (z. B. `curl http://localhost:8080/api/v1/lobbies`)
- Frontend (Vite-Dev-Server, `target=dev` ist der Default): `http://localhost:5173`
- `KASPA_NODE_URL` kann lokal unverändert auf `KASPA_USE_EXPLICIT_NODE=false` stehen bleiben
  (öffentlicher Resolver) oder auf eine bekannte Testnet-RPC-URL zeigen.
- Caddy startet **nicht** mit (kein `--profile proxy`) — `docker compose config --services` zeigt
  lokal nur `postgres backend frontend`, verifiziert.
- Wallet-Connect: Das Frontend nutzt das lokal vendorte `kaspa-wasm`-SDK und signiert im Browser;
  es reicht, im Browser ein Testnet-Wallet zu verbinden und eine Test-Balance/Deposit-Flow gegen den
  konfigurierten RPC-Endpunkt zu prüfen.

## 6. Was in diesem Zug geändert wurde (Config-Refactor)

- **`docker-compose.yml`**: Hartcodierte Kaspa-Node-IP (`ws://217.160.255.235:17110`) aus dem
  `backend`-Service entfernt — kommt jetzt ausschließlich aus `.env.production`. Frontend-URLs
  (`VITE_WS_BASE_URL`, `VITE_KASPA_NODE_URL`) und Caddy (`DOMAIN`, `TLS_EMAIL`,
  `KASPA_RPC_HTTP_TARGET`) sind jetzt über `${VAR}`-Substitution konfigurierbar statt fest auf
  `www.kaspabattle.com` verdrahtet. `caddy` läuft jetzt hinter `profiles: ["proxy"]`, damit ein
  einfacher `docker compose up` lokal nicht an einer fehlenden Domain/Caddyfile scheitert.
- **`battle-frontend/Dockerfile`**: `ARG`/`ENV` für `VITE_*` im `build`-Stage ergänzt — Vite bäckt
  diese Werte beim `npm run build` fest ins Bundle ein, reine Laufzeit-`environment:`-Variablen
  hätten für den `prod`-Target (statischer Build) nicht gereicht (nur für `dev`, wo der Vite-Dev-
  Server sie live liest). `docker-compose.yml` reicht sie jetzt korrekt als `build.args` durch.
- **`.env.docker.example`**: Kaspa-Sektion defaultet nicht mehr aktiv auf die feste externe IP
  (jetzt `KASPA_USE_EXPLICIT_NODE=false`, Resolver-Default); `CORS_ALLOWED_ORIGINS`, `DOMAIN`,
  `TLS_EMAIL`, `FRONTEND_BUILD_TARGET`, `VITE_*` ergänzt; Setup-Hinweis korrigiert (Datei wird
  tatsächlich als `.env.production` **und** `.env` benötigt, nicht nur `.env.docker`, das von
  `docker-compose.yml` nirgends gelesen wird).
- **`Caddyfile.example`**: Domain (`www.kaspabattle.com`), TLS-E-Mail und die externe Node-IP im
  `/kaspa-rpc`-Proxy-Block sind jetzt über Caddys `{$VAR}`-Platzhalter parametrisiert statt
  hartcodiert; der `/kaspa-rpc`-Block ist klar als optional markiert (empfohlen: RPC-Zugriffe über
  das Backend bündeln statt Browser→Node direkt).
- Rust-Code (CORS, RPC-Client) war bereits sauber env-var-getrieben — hier waren keine Änderungen
  nötig, nur die Compose-/Caddy-Ebene hatte die Hardcodes.

## 7. Merge-Zusammenfassung (`TournamentUpdate` → `main`)

- `main` war ein direkter Vorfahre von `TournamentUpdate` (`git merge-base` = `main`s vorherige
  Spitze) — der Merge war ein reiner **Fast-Forward ohne Konflikte**, keine manuelle
  Konflikt­auflösung nötig.
- Danach: `cargo build --workspace` (Debug-Profil) — **erfolgreich**, keine Fehler.
- `cargo test --workspace` — **209 Tests bestanden, 0 fehlgeschlagen, 2 ignoriert**.
- Kein Push nach `origin` durchgeführt (lokal 2 Commits vor `origin/main`) — bewusst, wie
  angewiesen.

## 8. Offene TODOs / Entscheidungen

- **Kaspa-RPC-Provider-Wahl**: Aktuell Resolver-Default; ein konkreter, verlässlicher externer
  wRPC-Provider (idealerweise `wss://`) für den Produktivbetrieb ist noch zu evaluieren.
- **SilverScript/Covenant-Rollout-Phasen**: siehe separate Analyse „Kaspa Battle × SilverScript —
  Integrationskonzept & Sicherheitsanalyse" — Phase 2/3 dort (Multisig-Härtung des
  Tournament-Escrows, danach Covenant-Testnet-Arbeit) ist unabhängig von diesem Docker-Setup und
  noch nicht begonnen.
- **Deposit-Double-Credit-Bug**: bereits als separater Hintergrund-Task gemeldet (`task_a0319e01`),
  hier nicht mit behoben — unabhängig vom Docker-/Merge-Umfang dieses Dokuments.
- **⚠ Kein tatsächlicher Docker-Image-Build wurde in dieser Session verifiziert.** `docker compose
  config`/`config --services` liefen erfolgreich (reine YAML-/Variablen-Validierung, braucht keinen
  laufenden Daemon). Der Versuch, `frontend` tatsächlich zu bauen, schlug fehl, weil **Docker
  Desktop auf dieser Maschine nicht läuft** (`failed to connect to the docker API ... check if the
  daemon is running`) — ein reines Umgebungsproblem dieser Session, keine inhaltliche Fehlermeldung
  zu den Dockerfile-/Compose-Änderungen. **Vor dem produktiven Rollout unbedingt selbst einmal
  `docker compose up --build` komplett durchlaufen lassen** (Docker Desktop/Docker Engine muss dafür
  laufen) und Health-Checks (`docker compose ps`) grün sehen — insbesondere die neuen `ARG`/`build.args`
  im Frontend-Dockerfile (Abschnitt 6) sind bisher nur durch Code-Review, nicht durch einen echten
  Build bestätigt.
- **Frontend-Healthcheck für `prod`-Target** nutzt weiterhin `curl` gegen `serve` auf Port 5173 —
  nicht separat erneut verifiziert, sollte aber unverändert funktionieren (Port/Server unverändert).
- Kein Monitoring (Grafana/Prometheus) im Compose-Setup enthalten — im Whitepaper erwähnt, aber
  nicht Teil der aktuellen Codebasis; bei Bedarf separat ergänzen.
