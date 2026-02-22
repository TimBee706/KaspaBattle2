# KaspaBattle Security Audit Report

**Version:** 1.0 | **Datum:** 22. Februar 2026  
**Auditor:** Senior Security Auditor (Repository-basiert)  
**Scope:** `c:\Projects\KaspaBattle2` — gesamtes Monorepo

---

## 9.1 Executive Summary

### Gesamtbewertung

> **⚠️ BEDINGT BESTANDEN — Nicht produktionsreif im aktuellen Zustand**

Das KaspaBattle-Repository zeigt eine solide Grundarchitektur und gut durchdachte Kernmodule (State Machine, BIP44-Wallet, PKCE-OAuth), enthält jedoch mehrere **CRITICAL**- und **HIGH**-Findings, die vor einem Mainnet-Launch zwingend behoben werden müssen.

### Findings nach Severity

| Severity | Anzahl |
|----------|--------|
| 🔴 CRITICAL | 3 |
| 🟠 HIGH | 5 |
| 🟡 MEDIUM | 6 |
| 🔵 LOW | 8 |
| ℹ️ INFORMATIONAL | 8 |

### Top-3-Risiken

1. **🔴 F-001 — Payout-TX ist ein Stub:** `execute_payout` in `payout.rs` gibt hartcodierte Mock-Transaktions-Hashes zurück. Es werden **keine echten KAS übertragen**.
2. **🔴 F-002 — resolve_match ohne Oracle-Authentifizierung:** Jeder kann `POST /api/matches/{id}/resolve` aufrufen und beliebige Gewinner eintragen.
3. **🔴 F-003 — submit_transaction gibt immer Fehler zurück:** `RealKaspaClient::submit_transaction()` gibt immer `Err(...)` zurück — kein Payout möglich.

---

## 9.2 Findings-Tabelle

| ID | Severity | Kategorie | Beschreibung | Datei:Zeile | Empfehlung |
|----|----------|-----------|--------------|-------------|------------|
| **F-001** | 🔴 CRITICAL | Unvollständige Kernfunktion | `execute_payout()` gibt `"mock_winner_tx"` und `"mock_treasury_tx"` zurück. Echte TX-Generierung fehlt. | `payout.rs:91-92` | TX-Generierung mit `kaspa-wallet-core` + UTXO-Auswahl + Signing implementieren. |
| **F-002** | 🔴 CRITICAL | Fehlende Zugriffskontrolle | `POST /api/matches/{id}/resolve` hat keinen Oracle-Auth-Guard. Jeder Aufrufer kann Gewinner setzen. | `routes.rs:626-784` | API-Key oder HMAC-JWT-Verifikation. Nur registrierte Oracle-Endpoints dürfen `resolve` aufrufen. |
| **F-003** | 🔴 CRITICAL | Permanente RPC-Fehler | `RealKaspaClient::submit_transaction()` gibt **immer** `Err(KaspaError::TransactionFailed(...))` zurück. | `rpc.rs:293-302` | Echte TX-Submission via `kaspa_rpc_core::api::rpc::RpcApi::submit_transaction()` implementieren. |
| **F-004** | 🟠 HIGH | Plaintext OAuth-Tokens in DB | `save_faceit_link()` speichert `access_token` und `refresh_token` unverschlüsselt. Code-Kommentar bestätigt dies explizit. | `faceit_oauth.rs:202-203` | AES-256-GCM Verschlüsselung (`aes-gcm` Crate). Master-Key aus Env-Variable. |
| **F-005** | 🟠 HIGH | Kein Rate-Limiting | Kein Rate-Limiting auf `/api/auth/login`, `/api/matches`, `/resolve`. Brute-Force und DoS möglich. | `main.rs`, `routes.rs` | `actix-governor` einbinden (z.B. max. 10 Login-Versuche/Min/IP). |
| **F-006** | 🟠 HIGH | Fehlender Timeout-Refund | Deposit-Timeout (30 Min) ist gesetzt, aber `watcher_task.rs` löst keinen automatischen Refund aus. | `watcher_task.rs`, `types.rs:10` | Abgelaufene Matches im `WaitingForDeposits`-State finden und Refund auslösen. |
| **F-007** | 🟠 HIGH | Mnemonic öffentlich zugänglich | `EscrowWallet::mnemonic_phrase()` ist `pub`. Ein Logging-Fehler kompromittiert die gesamte Schlüsselhierarchie. | `wallet.rs:87-89` | Auf `pub(crate)` beschränken. `zeroize` Crate nach Ableitung anwenden. |
| **F-008** | 🟠 HIGH | Deposit-Zuordnung nach Gesamtguthaben | `evaluate_deposits` prüft Gesamtbilanz statt TX-Sender. Ein Spieler kann beide Deposits erbringen. | `watcher.rs:67-85` | Individuelle UTXO-to-Sender-Zuordnung implementieren. |
| **F-009** | 🟡 MEDIUM | Disputed-State fehlt | `MatchState` hat keinen `Disputed`-State. Das Whitepaper beschreibt vollständigen Dispute-Prozess. | `match_state.rs:6-20` | `Disputed { reason, submitted_at }` State + Übergänge + Dispute-Endpoint. |
| **F-010** | 🟡 MEDIUM | Wallet-Index Kollision | `challenge_to_index()` mappt auf Bereich 0–1.048.576 (2^20). Birthday-Kollision bei ~1.200 Matches. | `wallet.rs:180-187` | Sequentiellen Zähler nutzen oder Range auf 2^31 + Kollisions-Check. |
| **F-011** | 🟡 MEDIUM | Keine CORS-Konfiguration | Kein CORS-Setup in Actix-Web sichtbar. CSRF und Cross-Origin-Angriffe möglich. | `main.rs` | `actix-cors` mit expliziter Origin-Whitelist. |
| **F-012** | 🟡 MEDIUM | Session-Token in JSON | Token wird als JSON zurückgegeben — Client muss ihn in `localStorage` halten (XSS-anfällig). | `auth.rs:105-118` | `Set-Cookie: HttpOnly; Secure; SameSite=Strict` verwenden. |
| **F-013** | 🟡 MEDIUM | Fehlende Kaspa-Adress-Validierung | `player_kaspa_address` wird bei Match-Erstellung nicht gegen Kaspa-Format validiert. | `routes.rs:38-44` | `kaspa_addresses::Address::try_from()` auf alle Input-Adressen erzwingen. |
| **F-014** | 🟡 MEDIUM | Migrations-Gap | Migrationen springen von `003_` auf `006_`. `004_users.sql` wird per `include_str!` geladen, kein `runner`. | `migrations/`, `auth.rs:30` | Migrationssequenz konsolidieren, `refinery`-Runner nutzen. |
| **F-015** | 🔵 LOW | Kein Node-Sync-Check | Vor Transaktionen wird nicht geprüft, ob der Kaspa-Node synchronisiert ist. | `escrow.rs`, `watcher.rs` | `is_synced()` vor kritischen Operationen aufrufen. |
| **F-016** | 🔵 LOW | Coinbase-Maturity ignoriert | `check_deposits()` summiert alle UTXOs inkl. unreifer Coinbase-UTXOs (< 100 DAA Scores). | `escrow.rs:199-216` | Coinbase-UTXOs unter Maturity-Schwelle ausfiltern. |
| **F-017** | 🔵 LOW | Schwache Passwort-Validierung | `validate_password()` prüft nur Mindestlänge ≥ 8. Keine Komplexitätsprüfung. | `auth.rs:43-48` | `zxcvbn` Crate für Stärkebewertung oder Komplexitäts-Regex. |
| **F-018** | 🔵 LOW | Account-Enumeration via Timing | `login()` ohne Hash-Berechnung bei nicht-existentem User — Timing-Seitenkanal. | `auth.rs:121-142` | Dummy-Argon2-Verifikation auch bei non-existentem User. |
| **F-019** | 🔵 LOW | Fehlende Content-Security-Policy | Kein CSP-Header im Vite-Frontend konfiguriert. | `vite.config.ts` | CSP via Reverse-Proxy-Header konfigurieren (`script-src 'self'`). |
| **F-020** | 🔵 LOW | Wallet-Cleanup fehlt | `useWalletStore` ohne `beforeunload`-Cleanup. WASM-Objekte bleiben im Speicher. | `useWalletStore.ts` | `window.addEventListener('beforeunload', disconnect)` + WASM-Cleanup. |
| **F-021** | 🔵 LOW | Hartcodierter Netzwerk-Fee | `tx_fee = 1000` Sompi ist hartcodiert statt dynamisch abgefragt. | `payout.rs:88` | `get_fee_estimate()` RPC-Methode implementieren. |
| **F-022** | 🔵 LOW | Wager-Typ-Inkonistenz | `wager_sompi` ist in DB `INTEGER` (i64), im Code `u64`. Potentieller Cast-Fehler bei großen Werten. | `routes.rs:267` | Explizite Prüfung auf nicht-negativen Wert nach DB-Auslese. |
| **F-023** | ℹ️ INFO | GamePlugin-Trait fehlt | Whitepaper §5.2 beschreibt Plugin-Architektur — nicht im Code. | Whitepaper §5.2 | Trait + CS2-Plugin als erste Implementierung. |
| **F-024** | ℹ️ INFO | Multi-Source Oracle fehlt | Nur FACEIT implementiert. Steam & Riot fehlen. | `oracle.rs` | Zweite Datenquelle + Konsens-Logik (2-von-3). |
| **F-025** | ℹ️ INFO | Oracle Staking/Slashing fehlt | Im Whitepaper beschrieben, nicht implementiert. | Whitepaper §3.3 | Phase-2-Feature; in Roadmap dokumentieren. |
| **F-026** | ℹ️ INFO | kdapp-Framework nicht integriert | Whitepaper §3.4 erwähnt kdapp-Episode-Trait — kein Crate im Repo. | Whitepaper §3.4 | Roadmap-Item; kdapp noch in Entwicklung. |
| **F-027** | ℹ️ INFO | Smart Contracts (Kasplex L2) fehlen | Korrekt als geplant kommuniziert, kein `contracts/`-Ordner. | Whitepaper §3.2 | Stand entspricht Whitepaper-Erwartungen. |
| **F-028** | ℹ️ INFO | Kein CI/CD | Keine GitHub Actions, kein Dockerfile vorhanden. | Repo-Root | CI/CD mit `cargo audit`, `clippy`, `tarpaulin`, `npm run test`. |
| **F-029** | ℹ️ INFO | Mnemonic im Speicher gehalten | Master-Mnemonic als `String` im Struct — lesbar in Memory-Dumps. | `wallet.rs:22-28` | `zeroize` Crate nach Schlüsselableitung anwenden. |
| **F-030** | ℹ️ INFO | Kein Wager-Matching in join_match | `join_match` prüft nicht, ob beitretender Spieler denselben Wager anbietet. | `routes.rs:387-500` | Wager aus DB mit Request-Wager vergleichen. |

---

## 9.3 Whitepaper-Compliance-Matrix

| # | Anforderung | Status | Anmerkung |
|---|-------------|--------|-----------|
| 1 | Match erstellen | ✅ | |
| 2 | Match beitreten | ✅ | |
| 3 | Deposit-Erkennung | ✅ | Zuordnung unscharf (F-008) |
| 4 | Status LOCKED | ✅ | |
| 5 | FACEIT-Integration | ✅ | Kein Multi-Source |
| 6 | Oracle-Ergebnisermittlung | ⚠️ | Kein Auth-Guard (F-002) |
| 7 | Auszahlung 95% Gewinner | ⚠️ | Stub-TX (F-001) |
| 8 | Blockchain-Dokumentation | ❌ | Keine echten TX-Hashes |
| 9 | State Machine vollständig | ⚠️ | DISPUTED fehlt (F-009) |
| 10 | Smart Contracts (MatchEscrow) | ❌ | Kasplex L2 ausstehend |
| 11 | Multi-Source Oracle | ❌ | Nur FACEIT (F-024) |
| 12 | Oracle-Staking/Slashing | ❌ | Geplant Phase 2 |
| 13 | Timeout-Refund | ⚠️ | Kein Auto-Trigger (F-006) |
| 14 | Dispute-Resolution | ❌ | Kein State/Endpoint (F-009) |
| 15 | Gebührenstruktur 5% | ✅ | |
| 16 | Wager-Grenzen 10–10.000 KAS | ✅ | |
| 17 | Ausschließlich natives KAS | ✅ | |
| 18 | FACEIT OAuth2 + PKCE | ✅ | |
| 19 | Kaspa WASM Wallet | ✅ | |
| 20 | GamePlugin-Architektur | ❌ | Kein Trait (F-023) |
| 21 | Kein eigener Token | ✅ | |
| 22 | DAO/Governance-Vorbereitung | ❌ | Keine Hooks |

**Gesamt: 11 ✅ / 5 ⚠️ / 6 ❌** — 50% der Whitepaper-Anforderungen vollständig implementiert.

---

## 9.4 Test-Coverage-Report

| Modul | Typ | Schätzung | Anmerkung |
|-------|-----|-----------|-----------|
| `auth.rs` | Unit | ~85% | Sehr gute Abdeckung |
| `match_state.rs` | Unit | ~80% | Happy-Path + negative Tests |
| `escrow.rs` | Unit | ~70% | Kein echter On-Chain-Test |
| `oracle.rs` | Unit+WireMock | ~75% | Gute Mock-Server-Tests |
| `wallet.rs` | Unit | ~90% | Inkl. Schnorr-Signatur |
| `watcher.rs` | Unit | ~70% | Deposit-Evaluierungs-Tests |
| `payout.rs` | Unit | ~50% | TX-Pfad nicht testbar (Stub) |
| `routes.rs` | — | ~0% | Keine Handler-Tests |
| `faceit_oauth.rs` | Unit | ~80% | PKCE-Flow gut getestet |
| Frontend | Unit | ~Unbekannt | Testdateien vorhanden |
| **Gesamt** | | **~55–65%** | Haupt-Gap: API-Handler ungetestet |

---

## 9.5 Empfehlungen (priorisiert)

### 🔴 Sofort (vor Alpha-Test)

1. **F-001**: Echte Kaspa-TX in `execute_payout()` — `kaspa_wallet_core::tx::Generator` nutzen
2. **F-002**: Oracle-Authentifizierung für `resolve_match`-Endpoint implementieren
3. **F-003**: `submit_transaction` in `RealKaspaClient` fertigstellen
4. **F-006**: Timeout-Refund-Automatismus im `watcher_task`
5. **F-008**: UTXO-to-Sender-Zuordnung statt Gesamtbilanz-Heuristik

### 🟠 Vor Beta-Launch

6. **F-004**: FACEIT-Tokens in DB mit AES-256-GCM verschlüsseln
2. **F-005**: Rate-Limiting mit `actix-governor`
3. **F-010**: Wallet-Index-Kollision via sequentiellen Zähler verhindern
4. **F-011 + F-012**: CORS-Konfiguration + httpOnly-Session-Cookies

### 🟡 Vor Mainnet-Launch

10. **F-009**: `Disputed`-State + vollständiger Dispute-Prozess
2. **F-013**: Kaspa-Adress-Validierung erzwingen
3. **F-015 + F-016**: Node-Sync-Check + Coinbase-Maturity-Filter
4. **F-018**: Timing-Angriff bei Login mitigieren
5. CI/CD-Pipeline mit `cargo audit`, `clippy`, `tarpaulin` aufsetzen

---

> [!CAUTION]
> Kein dynamischer Test (Penetrationstest, Fuzzing, Testnet-Deployment) wurde durchgeführt. Ein unabhängiges Sicherheitsaudit gegen ein Testnet-Deployment wird vor dem Mainnet-Launch dringend empfohlen.
