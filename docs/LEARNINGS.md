# Learnings & Decisions

Laufendes, geteiltes Protokoll nicht-offensichtlicher Erkenntnisse und Entscheidungen. Regeln zur Pflege: siehe [CLAUDE.md](../CLAUDE.md) → „Memory & Learnings".

**Format:** Neuer Eintrag **oben**. `### YYYY-MM-DD — Titel  [tag, tag]`, danach *Kontext / Erkenntnis / Konsequenz*. Tags z.B. `silverscript`, `escrow`, `kaspa-rpc`, `frontend`, `deploy`, `security`, `decision`. Nur, was aus Code/Git nicht ersichtlich ist.

---

### 2026-09-29 — refund_timeout doch interpreter-testbar: `this.ageDaa` = Input-`sequence`-Feld  [silverscript]

Korrektur einer eigenen Fehlannahme von vor ein paar Stunden (siehe Eintrag „10/10 Interpreter-Tests grün" unten): dort stand, `refund_timeout` sei ohne Simnet nicht testbar, weil `this.ageDaa` einen Live-Konsens-Kontext bräuchte. **Das stimmt nicht.** Beim Lesen von SilverScripts eigenem `compiler_tests.rs` (`compiles_require_age_daa_to_csv_and_verifies`) zeigt sich: `this.ageDaa >= N` lowert zu `OpCheckSequenceVerify` (Bitcoins BIP68/CSV-Äquivalent) und prüft einfach das `sequence`-Feld des ausgegebenen `TransactionInput` direkt — ein reiner Skript-Level-Check, kein Konsens-Lookup. Also mit demselben leichten Harness testbar wie alle anderen Entries: `sequence` im Test-Input auf den gewünschten Alterswert setzen.

Ergebnis: 2 neue Tests (`refund_timeout_succeeds_once_timeout_elapsed` mit `sequence == RESULT_TIMEOUT_DAA`, `refund_timeout_rejects_before_timeout_elapsed` mit `sequence == RESULT_TIMEOUT_DAA - 1`), beide grün beim ersten Versuch. **Jetzt 12/12 Interpreter-Tests, alle fünf Entry-Points abgedeckt.**

**Lehre für mich selbst:** Bevor ich „X ist ohne Y nicht testbar" ins Integrationsplan-Dokument schreibe, erst in der Zielbibliothek nach einem existierenden Test für genau dieses Feature suchen (`grep -n "ageDaa" **/*.rs`, ~30 Sekunden) statt aus der Architektur zu extrapolieren. Hier hätte ein früherer Blick in `compiler_tests.rs` (statt nur `chess_apps_tests.rs`) die Fehlannahme sofort vermieden.

### 2026-09-29 — Attestation-Referenz in Rust und TypeScript, cross-verifiziert  [silverscript]

`KASPABATTLE_RESULT_V1` (§8) jetzt zweimal implementiert und gegen denselben Testvektor geprüft:

- **Rust:** neues Crate `kaspabattle/battle-silverscript` (`attestation.rs`), Mitglied im bestehenden `kaspabattle`-Cargo-Workspace. Braucht nur `blake2b_simd` + `secp256k1` — beide schon im Workspace-`Cargo.lock` in exakt dieser Version vorhanden, kein Upgrade. Bewusst **ohne** `kaspa-txscript`/`kaspa-consensus-core`, da die den inkompatiblen neuen rusty-kaspa-Rev bräuchten. 7/7 Tests grün.
- **TypeScript:** eigenständiger Ordner `contracts/silverscript/tests/ts/` (eigenes `package.json`, **nicht** in `battle-frontend` eingehängt — noch nicht an den echten Signier-Fluss angebunden). `@noble/hashes` + `@noble/curves` (Blake2b/Schnorr) statt selbstgeschriebener Krypto — bewusste Entscheidung, weil dieser Code direkt über Auszahlungsziele entscheidet. Node 22.6+, `--experimental-strip-types`, kein Build-Schritt. 7/7 Tests grün.
- **Cross-Check bestätigt:** derselbe KAT-Digest (`dc848da9…acf90`) kommt aus dem echten interpretierten Contract (`interpreter_tests.rs`), der Rust-Referenz UND der TS-Referenz — unabhängig voneinander geschrieben, keine nachträglich aneinander angepasst.
- **Kleiner Fund:** `"KASPABATTLE_RESULT_V1"` hat 21 Zeichen, nicht 22 wie ein Kommentar/Test-Assert zunächst annahm — reiner Zählfehler (Test schlug prompt fehl, Ursache in 2 Minuten gefunden), keine Auswirkung auf die eigentliche Kodierung. In allen drei betroffenen Stellen korrigiert.

### 2026-09-29 — 10/10 Interpreter-Tests grün, zwei weitere reale Bugs gefunden (Gate 3 läuft)  [silverscript]

Fortsetzung des vorherigen Eintrags. Eigene Rust-Testsuite (`contracts/silverscript/tests/interpreter_tests.rs`, Muster von SilverScripts eigenem `chess_apps_tests.rs`) gegen den **echten Interpreter** (`TxScriptEngine`, `covenants_enabled: true`, reale Schnorr-Signaturen/Sighashes) — nicht nur Compiler-Check. Ergebnis: **10/10 Tests grün** für `join`, `cancel_unjoined`, `mutual_settle`, `oracle_settle` (je 1-3 Fälle inkl. gezielter Angriffe: Selbst-Beitritt, unterfinanzierter Output, Betragssumme falsch, gefälschte Oracle-Signatur, **umgeleiteter Gewinner-Payout trotz gültiger Attestation** — die zentrale „Output substitution"-Bedrohung aus dem Threat Model ist damit nicht nur auf Papier, sondern am echten Interpreter widerlegt). `refund_timeout` bleibt interpreter-ungetestet — `this.ageDaa` braucht einen Simnet-/Konsens-Kontext, den der leichte Test-Harness nicht bietet (SilverScripts eigene Suite testet `this.ageDaa`-Entries aus demselben Grund auch nicht so).

Auf dem Weg zwei weitere reale, vorher unbekannte Bugs gefunden und **vor** diesem Stand im Contract gefixt (Testergebnis zeigte erst generisches `VerifyError`, dann `CovenantsError(WrongGenesisCovenantId)` nach einem Fehlversuch, dann die richtige Ursache):

1. **`tx.outputs[idx].scriptPubKey` = 2-Byte-Big-Endian-Version + rohe Skript-Bytes**, nicht nur die rohen Bytes (verifiziert: `kaspa_txscript`s privater `SpkEncoding::to_bytes()`). Jede in-Contract gebaute P2PK-Vergleichs-Bytefolge (`mutual_settle`, `oracle_settle`, `refund_timeout`) brauchte ein vorangestelltes `byte[2](0x0000)`.
2. **`covenant_id` auf einem Output ist protokoll-bedeutsam, kein freies Tag.** Verifiziert gegen `kaspa_txscript::covenants::CovenantsContext::from_tx`: Output-`covenant_id` == Input-`covenant_id` → „Fortsetzung" (wird von `OpAuthOutputCount`/`OpAuthOutputIdx` gezählt, unabhängig vom `scriptPubKey`-Typ); jede Abweichung → „Genesis", verlangt einen aus dem gespenten Outpoint abgeleiteten Wert, sonst `WrongGenesisCovenantId` **und taucht in keinem Auth-Output-Kontext auf**. Terminale Payout-Outputs müssen deshalb dieselbe `covenant_id` wie das gespente Input-UTXO tragen — nicht, wie zunächst angenommen, eine je eigene/beliebige ID.
3. Nebenbefund: `actor_from_seed(0xFF)` (32× Byte `0xFF` als Secret Key) ist ein ungültiger secp256k1-Key (≥ Kurvenordnung) — beim Erzeugen deterministischer Test-Keys Bytewerte nahe `0xFF` meiden.

Aktualisierter Template-Hash nach beiden Fixes: `53acf11fd0d084048188db58b71277eb5eca2373a2a9e0068569caf6c2e8cc7d`. Werkzeug-Notiz: `librocksdb-sys` (Dev-Dependency von `kaspa-consensus`, für `cargo test` im SilverScript-Repo) braucht `libclang` — auf dieser Windows-Maschine fehlte das; behoben mit `winget install --id LLVM.LLVM` + `LIBCLANG_PATH="C:\Program Files\LLVM\bin"` (mit Timo abgestimmt vor der Installation).

### 2026-09-29 — Gate 2 freigegeben + erste kompilierende MatchEscrow-Contract (Gate 3 gestartet)  [silverscript]

- **Gate 2 (Contract-Spec + Threat Model) von Timo freigegeben**, inkl. zweier bestätigter Entscheidungen: kein Dispute-Entry-Point im MVP (nur die 5 vorgegebenen Entries), `mutual_settle` erhebt keine Plattform-Fee (nur `oracle_settle`).
- Vollständiger Entwurf in `docs/SILVERSCRIPT-INTEGRATION-PLAN.md` §7–11: State-Layout, 5 Entry-Points, `KASPABATTLE_RESULT_V1`-Attestation, Threat-Model, `MatchSettlement`-Trait/Manifest-Skizze.
- **Zwei durch echte Verifikation (nicht Annahme) korrigierte Design-Fehler, bevor Code entstand:**
  1. `int as byte[N]` in SilverScript lowert zu `OpNum2Bin`/`serialize_i64` — **Bitcoin-Style Little-Endian mit Vorzeichenbit im letzten Byte**, nicht big-endian wie zunächst angenommen. Rust-/TS-Referenzimplementierungen der Attestation müssen `serialize_i64` exakt nachbauen, kein naives `to_le_bytes()`.
  2. `this.ageDaa` ist keine normale vergleichbare Variable, sondern eine Grammatik-Sonderform, die **nur** `require(this.ageDaa >= expr)` zulässt (kein `<`, kein `&&`). Damit ist ein „Beitritt nur vor Ablauf"-Check (`join_timeout_daa` in `join`) **nicht ausdrückbar** — das Feld bleibt im MVP unenforced/reserviert.
- **`contracts/silverscript/match_escrow.sil` kompiliert erfolgreich** gegen den echten, lokal gebauten `silverc` (SilverScript `v1.0.0`@`3ed9733…`, rusty-kaspa @`a41a333b…`, lokal `rustc 1.95.0` — erfüllt die geforderte `rust-version = 1.94.0`). Bytecode 1213 Bytes, Template-Hash und 5 kollisionsfreie Dispatch-Tags dokumentiert in `contracts/artifacts/match_escrow.abi.json` + `match_escrow.manifest.json`.
- P2PK-`scriptPubKey` für Auszahlungsziele verifiziert gegen `kaspa-txscript::standard` @ gepinntem Rev: `OpData32 (0x20) || <32-Byte-Pubkey> || OpCheckSig (0xac)` — im Contract erfolgreich als `byte[1](0x20) + byte[](pk) + byte[1](0xac)` verwendet und kompiliert.
- **Noch offen (verbleibender Gate-3-Umfang):** kein Lauf gegen den echten Interpreter (`cli-debugger`/`TxScriptEngine`, `covenants_enabled: true`) für irgendeinen Entry-Point — die Kompilierung beweist nur Syntax/Typkorrektheit, nicht Laufzeitverhalten (vgl. Issue #252, das genau so einen Fehler nur zur Laufzeit zeigt). Rust-/TypeScript-Attestation-Referenzimplementierung mit Testvektoren (§8.3) ebenfalls noch nicht geschrieben.
- Werkzeug-Notiz: SilverScript-Compiler wurde lokal aus dem geklonten Repo gebaut (`cargo build -p silverscript-lang --bin silverc`, ~1:40 Min, zieht `kaspa-txscript`/`kaspa-consensus-core` etc. direkt per Git-Dependency vom gepinnten Rev).

### 2026-09-29 — Phase 2+3 (SilverScript-Quellen & Chess-Referenz) abgeschlossen  [silverscript]

Vollständiges Ergebnis in [SILVERSCRIPT-INTEGRATION-PLAN.md](SILVERSCRIPT-INTEGRATION-PLAN.md) (Abschnitte 3–6). Kernpunkte, alle direkt am geklonten/ausgecheckten Quellcode verifiziert (nicht aus Erinnerung):

- **Zu pinnende Revisionen:** SilverScript `v1.0.0` @ `3ed973335b59269293564805cc2c58a14595ec03`; rusty-kaspa @ `a41a333b08848f41bf737b72592e463a6011b8ac` (nicht `v2.1.0`/`master` — Issue #256 zeigt konkrete Build-Fehler von SilverScript v1.0.0 gegen v2.1.0).
- **Netzwerk endgültig geklärt:** Covenants/Toccata sind laut rusty-kaspa-Konsensparametern (`toccata_activation`) nur auf **testnet-10** (per DAA-Score, laut Issue #243 bereits aktiv, reale TX belegt) und Simnet (`always`) aktiv. **testnet-11/-12 werden von aktuellem rusty-kaspa nicht mehr unterstützt** (harter Panic bei `NetworkId::from`). Damit ist die alte offene Frage „testnet-10 vs. -12" für die SilverScript-Migration **zugunsten testnet-10** entschieden — unser Backend-Default `testnet-12` ist ohnehin schon fragwürdig (siehe Repo-Fallstricke-Eintrag) und mit aktuellem rusty-kaspa gar nicht betreibbar.
- **Reifegrad:** offizielle Doku (`docs.kaspa.org/toccata/silverscript`) sagt ausdrücklich, SilverScript sei „still moving toward its audited release interface" — kein Audit, keine Produktionsfreigabe. Passt zur Vorgabe „kein Mainnet".
- **Kritisches Compiler-Verhalten (Issue #252, offen, gut isoliert):** `readInputStateWithTemplate` (fremden Input lesen) + eigene `validateOutputState`-Fortsetzung in derselben Entry-Funktion crasht zur Laufzeit. Die echte Chess-Referenz umgeht das durchgängig (jede Entry liest entweder fremd und schreibt einen fremden Zieltyp, oder bleibt rein bei sich selbst) — das ist jetzt ein hartes Design-Constraint für den MatchEscrow-Contract in Gate 2.
- **State-Felder:** nur feste Breiten (`int`, `byte[N]`) — dynamische `byte[]`/`T[]` als State-Feld werden von `validateOutputState*` nicht unterstützt (Issue #168, deckt sich mit realem Chess-Code).
- **Hash-Funktionen nicht verwechseln:** State-Commitments in Chess nutzen `blake2b`; der Compiler selbst (Template-Hash, Dispatch-Tags) nutzt `blake3`.
- **TicTacToe:** keine kanonische/offizielle/Community-Implementierung gefunden (Repo-Suche + Web-Suche negativ) — transparent dokumentiert, Chess-App (`silverscript-lang/tests/apps/chess/`) ist die einzige verfügbare, real getestete Referenz.
- **Vendiertes `kaspa-wasm` (Frontend, v1.1.0-rc.3):** enthält keine Covenant-Symbole — vor Phase 7 (Wallet/Signing) prüfen, ob/welche neuere `kaspa-wasm`-Version SilverScript-kompatibel ist.
- **Compute-Budget:** kein Schätzer im SilverScript-Artefakt; muss aktuell per Trial-and-Error gegen den Node kalibriert werden (Issue #243) — für den Transaktions-Builder in Gate 2/3 einplanen.
- SilverScript-Repo wurde read-only nach `C:\Users\timob\AppData\Local\Temp\claude\...\scratchpad\silverscript` geklont (Tag `v1.0.0` ausgecheckt) — nicht Teil des KaspaBattle-Repos, nur Recherchequelle dieser Session.

### 2026-09-29 — SEC-MULTISIG-01 gefixt: Player-Key-Ableitung braucht jetzt Server-Secret  [escrow, security, fixed]

- **Fix für den Gate-1-Fund vom 2026-09-28** (siehe Eintrag darunter). `derive_player_key` in `kaspabattle/battle-kaspa/src/multisig/service.rs` nutzt jetzt `HMAC-SHA256(MULTISIG_KEY_DERIVATION_SECRET, "{match_id}-{role}-kaspabattle-multisig-v2")` statt reinem `SHA256(match_id-role-…)`. Neues Pflicht-Env `MULTISIG_KEY_DERIVATION_SECRET` (32 Byte hex), hartes Fail beim Start wie `ORACLE_PRIVATE_KEY` (`battle-api/src/main.rs`). `MultisigEscrowService::new()` hat dadurch einen neuen Pflichtparameter.
- Determinismus über Neustarts bleibt erhalten (`restore_escrow_from_row` leitet mit demselben Service/Secret neu ab; Player-Keys werden weiterhin nicht in der DB gespeichert).
- Neuer Regressionstest `test_player_keys_require_derivation_secret` (`multisig/service.rs`) beweist: gleiche Match-ID + gleicher Oracle-Key, aber unterschiedliches Secret → unterschiedliche Player-Keys/Escrow-Adresse; und die neue Ableitung reproduziert nicht mehr das alte, rein SHA256-basierte (öffentlich berechenbare) Schema.
- Verifiziert: `cargo build --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace --all-features` — alle grün (33/33 Multisig-Tests, gesamter Workspace ok).
- Neue Env-Var ergänzt in `kaspabattle/.env.example` und `.env.docker.example`; `docker-compose.yml` reicht sie automatisch über `env_file` durch (kein Compose-Change nötig). `docs/03-SECURITY.md` §Multisig Constructs dokumentiert Formel + Fix.
- **Bleibt bewusst bestehen:** Modell ist weiterhin custodial (Server leitet alle drei Rollen ab/hält den Oracle-Key) — das ist der dokumentierte Ist-Zustand vor der SilverScript-Migration, kein neuer Fund. Nicht verwechseln: das ist ein anderes Problem als „Server kompromittiert" — der gefixte Bug erlaubte Diebstahl **ohne** Server-Kompromittierung, nur mit der öffentlichen Match-ID.
- **Nicht behoben (Scope):** `battle-kaspa/src/escrow.rs::derive_escrow_address` (Legacy-Single-Key-Pfad, SHA256 aus öffentlichen Werten) hat dasselbe Muster, wird aber nur in Tests referenziert und ist nicht der aktive Turnier-/Match-Payout-Pfad (siehe Gate-1-Befund) — bei Bedarf separat prüfen, bevor dieser Pfad reaktiviert wird.

### 2026-09-28 — Gate-1-Befund: Ist-Zustand Escrow/Wallet (main @ a955d4f)  [escrow, security]

- **Alle Branches sind in `main` enthalten** (`TournamentUpdate`, `Codereview`, `fix/faceit-stats-and-lobby-stability`, alte Feature-Branches; ahead 0). Die Admin-/Dispute-/Audit- und Wallet-Deposit-Commits (b1543b0, f22dbe9) sind gemergt. Simulierte Deposits (`simulated_tx_*`) sind seit f22dbe9 entfernt.
- **Kritisch, gefixt 2026-09-29 (siehe Eintrag oben):** `MultisigEscrowService::derive_player_key` (`battle-kaspa/src/multisig/service.rs:759`) leitete beide Spieler-Keys als `SHA256("{match_id}-{role}-kaspabattle-multisig")` ab. Die Match-ID ist öffentlich (`GET /lobbies`, `/matches/:id` ohne Auth). Damit waren 2 von 3 Keys jedem bekannt → das „2-of-3" schützte nichts, unabhängig vom Oracle-Key. Zusätzlich ist das Modell ohnehin backend-custodial (Server hält Oracle-Key aus `ORACLE_PRIVATE_KEY`, signiert alles; PSKT-Version `1.0-backend-held`; die „Winner-Signatur" ist nur eine Auth-Nachricht) — das bleibt so bis zur SilverScript-Migration.
- Turnier-Escrow: HD-Key aus `KASPA_MNEMONIC` auf dem Server, Payout/Refund serverseitig; Admin-Endpunkte nur per statischem `X-Admin-Api-Key`.
- Mnemonic liegt im Klartext im Zustand-Store (`useWalletStore.account.mnemonicPhrase`) und wird pro Sendung neu abgeleitet. Store ist nicht persistiert, aber XSS/Extension-exponiert.
- Netzwerk-Default gespalten: Backend `testnet-12` (`main.rs`, `api/mod.rs`), Frontend/Docker `testnet-10`; beide nutzen Präfix `kaspatest:` → falsche Netzwerkwahl fällt bei Adressen nicht auf.
- Dependencies: Backend `kaspa-*` **0.15.0 von crates.io** (keine Git-Revs im Cargo.lock); vendiertes `kaspa-wasm` **1.1.0-rc.3** ohne Commit-Provenienz, enthält keinerlei `covenant`-Symbole in `kaspa.d.ts`. Covenant-/SilverScript-Kompatibilität ist damit ungeklärt (Phase 2).
- `battle-kaspa/src/escrow.rs::derive_escrow_address` (SHA256 aus öffentlichen Werten) wird nur in Tests genutzt; Doku-Kommentare dort nennen noch „Kasplex L2" als Ziel (widerspricht L1-only).

### 2026-09-28 — Zielrichtung: SilverScript, reines Layer 1  [silverscript, decision]

- **Entscheidung (Timo):** SilverScript wird künftig eingeführt; KaspaBattle bleibt ein reines L1-Projekt. Keine L2-/Rollup-/Bridge-Ansätze.
- **Stand im Repo:** Roadmap in [03-SECURITY.md](03-SECURITY.md) (v1.0 custodial → v1.5 Multisig-PSKT → v2.0 kdapp-Episode → v3.0 SilverScript/Covenants) und Konzeptskizze in [06-KASPA-BATTLE-INTEGRATION.md](06-KASPA-BATTLE-INTEGRATION.md). Die Docs nennen „Testnet 2026-2027" — **unverifiziert**.
- **Offen:** Die in `DEPLOYMENT_IONOS_DOCKER.md` erwähnte Analyse „Kaspa Battle × SilverScript — Integrationskonzept & Sicherheitsanalyse" liegt nicht im Repo. Aktuellen SilverScript-Stand (Syntax, Opcodes, Testnet) recherchieren, bevor Code entsteht.
- **Konsequenz:** Escrow-/Payout-Logik hinter einer austauschbaren Schnittstelle halten (`EscrowService` / `MultisigEscrowService`), damit ein Covenant-Backend später einsteckbar ist.

### 2026-09-28 — Repo-Fallstricke  [deploy, frontend]

- Netzwerk uneinheitlich konfiguriert (`testnet-10` in Docker-/Frontend-Config, `testnet-12` in Backend-`.env.example`/Docs) — vor Netzwerk-Arbeit vereinheitlichen.
- Docs verlinken `04-EPISODE-FRAMEWORK.md`, die nicht existiert.
- `kaspabattle/target/.rustc_info.json` ist versehentlich getrackt und verschmutzt `git status`.
- Volltext-Suche im Repo-Root timeoutet (`target/`, `node_modules/`) → `git grep` nutzen.
