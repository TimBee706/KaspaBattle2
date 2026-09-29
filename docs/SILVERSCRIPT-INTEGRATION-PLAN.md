# SilverScript L1 Escrow — Integration Plan

Status: **Phase 0–3 abgeschlossen, Gate 1 freigegeben (2026-09-29). Gate 2 (Contract-Spec + Threat Model, Abschnitte 7–11) von Timo freigegeben (2026-09-29, Dispute-Scope und Fee-Modell wie entworfen bestätigt). `match_escrow.sil` kompiliert erfolgreich UND besteht 10/10 Tests gegen den echten Interpreter für `join`/`cancel_unjoined`/`mutual_settle`/`oracle_settle` (Abschnitt 7.7/7.8, zwei reale Bugs dabei gefunden und gefixt). Noch offen: `refund_timeout` interpreter-/simnet-testen, Attestation-Referenzimplementierung (8.3), dann Gate 3 (echte Testnet-TX) selbst.**
Branch: `feature/silverscript-l1-escrow`. Scope: nur 1v1-Matches, Turniere werden nicht migriert.

Diese Datei wird mit jeder Phase weitergeschrieben (siehe [CLAUDE.md](../CLAUDE.md) → „Memory & Learnings"). Abschnitte ab „Contract-State" sind Platzhalter, bis Gate 2 ansteht.

---

## 1. Executive Summary

KaspaBattle will Wager-Escrow perspektivisch vollständig auf Kaspa L1 abbilden, sobald **SilverScript** (die Covenant-Sprache für den **Toccata**-Hardfork, [KIP-17](https://github.com/kaspanet/kips/blob/master/kip-0017.md)) dafür reif ist. Der aktuelle Stand (2026-09-29):

- SilverScript liegt als `v1.0.0` vor (kaspanet/silverscript, Tag `v1.0.0` @ `3ed973335b59269293564805cc2c58a14595ec03`), ist aber laut offizieller Doku **„still moving toward its audited release interface"** — ausdrücklich noch nicht produktionsreif/auditiert.
- Der Compiler pinnt rusty-kaspa auf einen festen Git-Rev (`a41a333b08848f41bf737b72592e463a6011b8ac`), **nicht** auf den neuesten Tag (`v2.1.0`) — und `silverscript-abi` v1.0.0 baut nachweislich **nicht** gegen v2.1.0 (Issue #256, offen). Wir übernehmen denselben Rev-Pin, nicht „master" oder „latest".
- Unser eigenes Backend hängt an `kaspa-* = "0.15"` (crates.io) — einer deutlich älteren rusty-kaspa-Linie ohne Covenant-Opcodes/`CovenantsContext`. Ein SilverScript-Backend braucht also einen **separaten, neueren rusty-kaspa-Abhängigkeitsbaum**, der neben dem bestehenden 0.15er-Baum koexistiert (siehe Abschnitt 4/6).
- Konsensdaten aus rusty-kaspa @ `a41a333b`: `covenants_enabled` ist nur auf **testnet-10** (feste DAA-Score-Aktivierung) und **Simnet** (`always`) aktiv; **testnet-11/-12 werden von aktuellem rusty-kaspa gar nicht mehr unterstützt** (`panic!("Testnet suffix {} is not supported")`). Devnet hat `never`. Ein reales, akzeptiertes TN10-Covenant-TX-Beispiel ist in Issue #243 dokumentiert — Covenants laufen also bereits produktiv auf TN10.
- **Konsequenz für unser Netzwerk-Wirrwarr** (siehe `project-open-issues-and-inconsistencies`-Memory): **testnet-10 ist die einzig tragfähige Wahl**, auch unabhängig von SilverScript. Unser Backend-Default `testnet-12` ist mit aktuellem rusty-kaspa nicht mehr betreibbar.
- Es existiert **keine kanonische, offizielle TicTacToe-Implementierung** in SilverScript (weder im Repo noch über GitHub-/Web-Suche auffindbar). Referenzimplementierung ist wie im Auftrag vorgesehen die **Chess-App** (`silverscript-lang/tests/apps/chess/`), die real existiert, real kompiliert und real gegen den Kaspa-Script-Interpreter getestet wird (`kaspa_txscript::TxScriptEngine`, `EngineFlags{covenants_enabled:true}`).
- Ein konkreter, gut isolierter Compiler-Bug (**Issue #252**, offen) verbietet eine bestimmte Kombination (`readInputStateWithTemplate` + eigene `validateOutputState`-Fortsetzung in derselben Entry-Funktion) — das reale Chess-Referenzdesign umgeht dieses Muster bewusst, und unser MVP-Design muss es ebenfalls umgehen (siehe Abschnitt 6).

---

## 2. Repository-Befund (Gate 1 — Kaspa-seitig, bereits umgesetzt)

Vollständiger Befund in `docs/LEARNINGS.md` (Einträge „Gate-1-Befund" und „SEC-MULTISIG-01 gefixt"). Kurzfassung:

- Alle Feature-Branches sind in `main` gemerged (siehe Branch-Matrix unten); simulierte Deposits sind entfernt, echte Wallet-TX sind aktiv.
- **SEC-MULTISIG-01 (kritisch, gefixt 2026-09-29):** Multisig-Player-Keys waren rein aus der öffentlichen Match-ID ableitbar → jeder konnte 2 von 3 Signaturen ohne Server-Kompromittierung erzeugen. Fix: `HMAC-SHA256(MULTISIG_KEY_DERIVATION_SECRET, …)` statt `SHA256(match_id-role-…)`, neues Pflicht-Secret, Regressionstest, alle Tests grün.
- Aktueller Escrow-Pfad (`battle-kaspa/src/multisig/`) bleibt bestehen (Legacy-Fallback), wird durch dieses Projekt **nicht entfernt**.
- Frontend-Wallet hält die Mnemonic im Klartext im Zustand-Store (`useWalletStore`), leitet pro Sendung neu ab; kein Persistieren, aber XSS-exponiert während der Session.

---

## 3. Branch-Matrix (Phase 0)

Stand 2026-09-29, `git fetch --all --prune` + `git rev-list --left-right --count main...<branch>`:

| Branch | Ahead/Behind main | Zweck | Gemergt? | Noch relevante Dateien | Security-Risiken | Empfehlung |
|---|---|---|---|---|---|---|
| `TournamentUpdate` (+ `origin/TournamentUpdate`) | 5 / 0 | Turnier-Feature-Entwicklung (Admin, Dispute, Audit, Wallet-Deposit) | **Ja**, vollständig in `main` (bis `f22dbe9`) | keine (main ist superset) | keine (historisch) | ignorieren / löschen |
| `Codereview` (+ `origin/Codereview`) | 54 / 0 | Code-Review-Notizen-Branch | Ja | keine | keine | ignorieren / löschen |
| `origin/fix/faceit-stats-and-lobby-stability` | 55 / 0 | ESLint-Fix (FACEIT-Stats-Typing) | Ja | keine | keine | ignorieren / löschen |
| `feature/ui-polish-full-site-pass` | 1 / 0 | UI-Polish (aktueller lokaler Branch vor diesem Projekt) | Ja | keine | keine | ignorieren (lokaler Rest) |
| `feature/public-beta-landingpage` | 2 / 0 | Public-Beta-Landingpage | Ja | keine | keine | ignorieren |
| `MultisigUpdate`, `RefundUpdate`, `FaceitWatcher/Payout`, `GoLiveUpdate`, `MobileUpdate`, `ValorantIntegration`, `kaspa-battle-ui-update` | 63–145 / 0 | historische Feature-Branches | Ja (Remote-Tracking bereits `origin`-seitig gelöscht — `[gone]`) | keine | keine | lokal löschen (`git branch -d`) |
| `main` / `origin/main` @ `a955d4f` | — | Produktivstand | — | — | — | Basis für `feature/silverscript-l1-escrow` |

**Ergebnis:** Es gibt **keinen** offenen Merge-Konflikt oder nicht gemergten Sicherheits-relevanten Code. Alle Branches sind entweder in `main` enthalten oder veraltete, bereits vom Remote gelöschte Nebenzweige. Keine Cherry-Picks nötig. Empfehlung: lokale Alt-Branches bei Gelegenheit aufräumen (nicht Teil dieses Auftrags, nur zur Kenntnis).

---

## 4. Bestehende Kaspa-Architektur (Ist-Zustand)

Siehe `docs/01-ARCHITECTURE.md`, `docs/02-KASPA-INTEGRATION.md`, `docs/03-SECURITY.md` für die volle Beschreibung. Für diese Migration relevant:

- **Frontend:** `battle-frontend/src/kaspa/wallet.ts` hält Mnemonic im Zustand-Store, leitet Schlüssel deterministisch (BIP44, `kaspa-wasm` v1.1.0-rc.3) ab, signiert lokal im Browser über `kaspa.PrivateKeyGenerator` + `pending.sign()`. Reale Deposits (`sendDeposit`) sind seit `f22dbe9` aktiv, keine Simulation mehr.
- **Backend:** `battle-kaspa` (kaspa-* 0.15 von crates.io) betreibt `EscrowService` (Legacy, Single-Key/HD) und `MultisigEscrowService` (2-of-3 P2SH, PSKT-artig, aber backend-custodial). Netzwerk-Default ist im Code gespalten zwischen `testnet-10` (Frontend/Docker) und `testnet-12` (Backend/Docs) — **siehe Abschnitt 1: testnet-12 ist mit aktuellem rusty-kaspa nicht mehr unterstützt.**
- **RPC:** `battle-kaspa/src/rpc.rs::RealKaspaClient` nutzt den Kaspa-Resolver oder eine explizite `KASPA_NODE_URL`, mit Retry/Backoff. Kein eigener Node erforderlich (deckt sich mit Phase-8-Vorgabe).
- **Vendiertes `kaspa-wasm`:** Version `1.1.0-rc.3`, ohne Commit-Herkunft im Repo dokumentiert, **keine Covenant-Symbole** in `kaspa.d.ts` (geprüft: 0 Treffer für „covenant"). Für clientseitiges SilverScript-Signing (Phase 7) muss geprüft werden, ob eine neuere `kaspa-wasm`-Version Covenant-fähige Transaktionsbau-Primitive mitbringt, oder ob wir TX-Bau serverseitig/im Debugger-Stil nachbauen müssen.

---

## 5. SilverScript — was es ist, Versionen, Kernprimitive (Phase 2)

### 5.1 Herkunft & Reifegrad

- Repo: `kaspanet/silverscript` (öffentlich, ISC-Lizenz, erstellt 2026-01-29, 56 Stars, 41 offene Issues, aktiv gepflegt).
- Konsens-Grundlage: **[KIP-17](https://github.com/kaspanet/kips/blob/master/kip-0017.md)** „Covenants and Improved Scripting Capabilities" (Status: Active, Autor Ori Newman, Fortsetzung von KIP-10), führt Introspektions-Opcodes (`OpTxInput*`, `OpTxOutput*`, `OpOutpointTxId/Index`, …), `OpCat`/`OpSubstr`, `OpCheckSigFromStack(ECDSA)`, `OpBlake2bWithKey`, `OpBlake3(WithKey)` ein. Der Hardfork-Codename dafür ist **„Toccata"**.
- Offizielle Doku (`docs.kaspa.org/toccata/silverscript`): SilverScript ist „the primary development language for writing Toccata covenants" — **aber ausdrücklich**: „Silverscript is still moving toward its audited release interface. Treat it as the intended covenant authoring direction, but check the repo and release notes before shipping production value." → Kein Audit, keine Produktionsfreigabe.
- **Zu verwendender Commit/Tag: `v1.0.0` @ `3ed973335b59269293564805cc2c58a14595ec03`** (2026-09-09, "Prepare SilverScript 1.0 and clarify ABI artifact validation"). Nicht `master` verwenden (Anforderung des Auftrags), zumal `master` seit `v1.0.0` bereits weitergelaufen ist.
- **Kompatible rusty-kaspa-Revision: `a41a333b08848f41bf737b72592e463a6011b8ac`** (Workspace-`Cargo.toml`-Pin, mainline `kaspanet/rusty-kaspa`, August 2026, vor `kaspa-txscript-zk-sdk`-Erweiterungen). **Nicht** der neueste Tag `v2.1.0` — Issue #256 (offen, 2026-09-26) dokumentiert konkrete Compile-Fehler von `silverscript-abi` v1.0.0 gegen `v2.1.0` (`EngineFlags` ohne `covenants_enabled`-Feld mehr, geänderte `deserialize_i64`-Signatur, umbenannte Config-Felder — v2.1.0 hat den „pre-Toccata"-Modus entfernt).

### 5.2 Netzwerk-/Aktivierungsstatus (aus rusty-kaspa-Quellcode @ `a41a333b`, `consensus/core/src/config/params.rs`)

| Netzwerk | `toccata_activation` | Bedeutung |
|---|---|---|
| Mainnet | `ForkActivation::new(474_165_565)` | fester zukünftiger DAA-Score, noch nicht erreicht |
| **Testnet-10** | `ForkActivation::new(467_579_632)` | fester DAA-Score — **laut Issue #243 (reale, akzeptierte TN10-TX) bereits aktiv** |
| Testnet-11/-12 | — | `NetworkId::from` **paniced** mit „Testnet suffix … is not supported" — in aktuellem rusty-kaspa gar nicht mehr wählbar |
| Simnet | `ForkActivation::always()` | immer an — geeignet für lokale Entwicklung/CI |
| Devnet | `ForkActivation::never()` | Covenants hier nie aktiv |

→ Für jede weitere SilverScript-Arbeit: **testnet-10**, nicht testnet-12 (aktueller Backend-Default) oder testnet-11.

### 5.3 Werkzeuge

- `silverc` (CLI-Compiler): `.sil` → JSON-Artefakt (ABI/Bytecode), optional `--ast-only`, `--constructor-args <json>`, `-o`/`-c`.
- `cli-debugger` / `sil-debug`: quelltextnahes Debugging mit Test-Dateien (`--test-file … --run-all`), führt reale Test-Vektoren gegen den Compiler+VM-Stack.
- ABI-Crate `silverscript-abi`: `SilAbiArtifact { schema_version: u32 (aktuell 1), structs, contracts }`, `CompiledContractArtifact { entries, template_hash: [u8;32], … }`.
- **`template_hash(prefix, suffix)` nutzt Blake3** (nicht SHA-256/Blake2b!): `Blake3(len(prefix) || prefix || len(suffix) || suffix)`, mit 8-Byte-Längenpräfixen. Wichtig für Manifest-Validierung (Phase 6).
- **Dispatch-Tag** je Entry-Funktion: 4 Bytes = erste 4 Bytes von `Blake3("entry_name(typ1,typ2,…)")` — ein Solidity-Selector-Analogon, aber mit Blake3 statt Keccak. Kollisionen sind ein Compile-Fehler (`EntrypointDispatchTagCollision`).

### 5.4 Kernprimitive (aus realem Chess-Quellcode verifiziert, nicht aus Doku übernommen)

| Primitiv | Zweck |
|---|---|
| `this.activeInputIndex` | Index des gerade validierten Inputs |
| `this.ageDaa` | relatives Alter (DAA-Ticks) des ausgegebenen UTXO — Basis für Timeouts (`require(this.ageDaa >= move_timeout)`) |
| `OpInputCovenantId(idx)` / `OpCovInputCount(id)` / `OpCovInputIdx(id, n)` | Covenant-Input-Gruppierung: mehrere Inputs, die zusammen eine atomare Transition bilden |
| `OpAuthOutputCount(idx)` / `OpAuthOutputIdx(idx, n)` | welche Outputs ein gegebener Input autorisieren darf |
| `readInputStateWithTemplate(idx, prefixLen, suffixLen, templateHash)` | liest den State eines **fremden** Inputs, verifiziert dessen Skript-Form über den Template-Hash |
| `validateOutputState(idx, NewState)` | prüft, dass Output `idx` den eigenen Contract mit `NewState` fortsetzt (gleiches Template) |
| `validateOutputStateWithTemplate(idx, NewState, prefix, suffix, templateHash)` | wie oben, aber für einen **fremden** Zieltyp (Konstruktor-fixierter Prefix/Suffix) |
| `validateOutputStateWithInputTemplate(idx, NewState, otherInputIdx, prefixLen, suffixLen, templateHash)` | wie oben, aber Prefix/Suffix vom Template eines anderen Inputs übernommen |
| `templateHash(prefix, suffix)` (Sprachfunktion) | berechnet den Template-Hash direkt im Contract (z. B. um ihn On-Chain gegen eine Commitment zu prüfen) |
| `checkSig(sig, pubkey)` + `blake2b(pubkey) == owner`-Commitment | Owner-Signaturen — Pubkey wird nie im State gespeichert, nur sein Hash |
| `OpOutpointTxId`/`OpOutpointIndex` | eindeutige IDs aus dem ausgegebenen Outpoint ableiten (kein mutabler Zähler nötig) |

**Wichtig:** `blake2b`, nicht `sha256`, ist die durchgängige Hash-Funktion für State-Commitments in Chess; `template_hash`/Dispatch-Tags im Compiler selbst nutzen `blake3`. Beides ist bewusst zu unterscheiden.

### 5.5 Bekannte, verifizierte Compiler-/Debugger-Limits (aus den genannten Issues, alle **offen** zum 2026-09-29)

| Issue | Befund | Auswirkung auf unser Design |
|---|---|---|
| **#252** | `readInputStateWithTemplate` (oder jede „fremdes Template lesen"-Primitive) **plus** eine eigene `validateOutputState`-Fortsetzung **in derselben Entry-Funktion** crasht zur Laufzeit (`-N cannot be used as an array index`), unabhängig von Reihenfolge/Schleife. Gut isolierter Minimal-Repro vorhanden; vermutete Ursache ein nicht konvergierter `bytecode_size`-Fixpunkt im Compiler. | **Hartes Design-Constraint**: Eine Entry-Funktion darf entweder (a) einen fremden Input lesen und einen fremden/anderen Output-Typ fortsetzen (wie `ChessSettle::settle`, `Player::start_game`), oder (b) nur die eigene State fortsetzen — **nicht beides gemischt**. Unser `oracle_settle`/`mutual_settle` MVP-Design muss diesem Muster folgen (siehe Chess: `mux` liest nie fremd + committed nie eigenen Typ gleichzeitig; `settle` liest fremd, schreibt fremden Zieltyp). |
| **#256** | `silverscript-abi` v1.0.0 baut nicht gegen rusty-kaspa v2.1.0 (`EngineFlags`, `deserialize_i64`-Signatur, `max_signature_script_len`-Umbenennung). | Bestätigt: **exakt den gepinnten Rev `a41a333b` verwenden**, nicht `v2.1.0`/`master`. |
| **#243** | Kein Compute-Budget-Schätzer im Artefakt; nötiges `computeBudget`-Commitment pro Input muss aktuell per „senden → Ablehnung lesen → Zahl übernehmen" ermittelt werden (TN10, reale TX-IDs als Beleg). | Unser Transaktions-Builder (Phase 6/8) braucht einen Trial-and-Error- oder Kalibrierungsschritt für das Compute-Budget je Entry-Point — kein fixer Wert vorab bekannt. |
| **#168** | `T[]` (dynamische Arrays) als **State-Feld** in `validateOutputState*` wird nicht unterstützt; nur feste Breiten. | Deckt sich mit dem realen Chess-Code: State-Structs verwenden ausschließlich `int`/`byte[N]` fester Länge (z. B. `byte[288] route_templates`, `byte[64] board`) — unser `MatchEscrow`-State darf ebenfalls keine dynamischen Arrays enthalten. |
| **#139** | Doku-Drift: `state { … }`-Block aus TUTORIAL.md/vprogs.xyz wird vom Compiler abgelehnt; tatsächliche Syntax sind bare, typisierte Felddeklarationen im Contract-Body (wie im realen Chess-Code gesehen). Weitere Encoding-Fallstricke bei `readInputStateWithTemplate` in N:M-Covenants. | **Nie aus TUTORIAL.md/DECL.md unverifiziert übernehmen** — immer gegen reale `.sil`-Dateien/Tests im ausgecheckten Repo gegenprüfen (Auftragsvorgabe „keine ungeprüften Codebeispiele" bestätigt sich hier konkret). |
| **#218**, **#242**, **#253**, **#254** | Diverse Performance-/Tooling-Feinheiten (Bytecode-Größensensitivität komponierter Covenants, irreführende Fehlermeldungen bei Argument-Mismatches, `cli-debugger`-Encoding-Lücken). | Für MVP nachrangig, in Testplan (Gate 3) berücksichtigen. |

---

## 6. Analyse: Chess-Referenz (Phase 3) — keine kanonische TicTacToe gefunden

**Transparenter Befund:** Trotz Suche im SilverScript-Repo, per GitHub-Code-/Repo-Suche und Web-Suche wurde **keine offizielle, experimentelle oder Community-TicTacToe-Implementierung in SilverScript gefunden.** Referenz ist wie im Auftrag für diesen Fall vorgesehen die **Chess-App**, die real existiert unter `silverscript-lang/tests/apps/chess/` (v1.0.0) und über `silverscript-lang/tests/chess_apps_tests.rs` (2034 Zeilen) gegen den echten Kaspa-Skript-Interpreter (`kaspa_txscript::TxScriptEngine`, `EngineFlags{covenants_enabled:true}`) getestet wird — kein Community-Demo, sondern Teil der offiziellen Testsuite.

Contracts: `League` (Registrierung), `Player` (Account/Rating/Spielstart), `ChessMux` (laufendes Spiel, Zugvalidierung, Timeout), sieben Zug-„Worker" (`chess_pawn/knight/vert/horiz/diag/king/castle(_challenge)`), `ChessSettle` (Auszahlung + Elo-Update).

| Pattern | Verwendung in Chess | Für KaspaBattle geeignet | Risiko | Vereinfachung für MVP |
|---|---|---|---|---|
| Template-Hashes (`*_template`, `templateHash()`) | Jeder Contract-Typ hat einen Hash-Selektor, der ihn eindeutig identifiziert, ohne seinen Bytecode hart zu verdrahten | Ja — genau das Manifest-Konzept aus Phase 6 (Template-Hash im Manifest) | gering | Für 1v1-MVP reichen 2–3 Templates (MatchEscrow selbst, evtl. ein Settle-Typ) statt 9 wie bei Chess |
| Route-Commitments (`routes_commitment`, gepacktes `byte[288]`) | Konstruktor committet sich auf alle erlaubten Folge-Contract-Templates gebündelt als ein Hash | Bedingt — nur nötig, wenn ein Contract in mehrere strukturell verschiedene Folgezustände verzweigen kann | mittel (Komplexität) | MVP hat wenige, feste Übergänge (`join`→`mutual_settle`/`oracle_settle`/`refund_timeout`) — eher direkte Templates statt gepackter Routen-Tabelle |
| `readInputStateWithTemplate` / `validateOutputState(WithTemplate)` | Kernmechanismus für Cross-Contract-Zustandsübergänge | Ja, zentral für `MatchEscrow` | **hoch — Issue #252** (siehe 5.5): fremd lesen + eigen fortsetzen in einer Entry crasht | MVP-Entries so schneiden wie Chess: entweder rein eigene Fortsetzung (z. B. `join`) oder rein fremde Ziel-Templates ansprechen (z. B. ein `settle`, das nur Auszahlungs-Outputs prüft) |
| Covenant-Input-Grouping (`OpInputCovenantId`/`OpCovInput*`) | Bindet mehrere Inputs (z. B. beide Spieler + Settle-Worker) atomar zusammen | Ja — nötig, damit `oracle_settle`/`mutual_settle` beide Spieler-Outputs in einer TX korrekt zuordnen | mittel | 1v1 hat nur 2 Parteien statt Chess' N Spieler/Workers — einfacherer Gruppen-Fall |
| Authorized-Output-Mapping (`OpAuthOutputCount/Idx`) | Bindet genau welche Outputs ein Input autorisieren darf | Ja, direkt übernehmbar | gering | 1:1 übernehmbar |
| Delegate-Entrypoints (`delegate_start_game`, `delegate_settle`) | Nicht-Leader-Inputs einer Covenant-Gruppe validieren nur, dass sie wirklich Teil der Gruppe sind, produzieren aber selbst keine Outputs | Ja — Player B als „Delegate" in einer von Player A geleiteten TX | mittel | Für MVP evtl. verzichtbar, wenn wir symmetrisches Zwei-Parteien-Design ohne Leader/Delegate-Asymmetrie wählen — zu entscheiden in Gate 2 |
| Settlement-Separation (`ChessMux` → `ChessSettle` als eigener Contract-Typ) | Auszahlung/Rating-Update ist bewusst ein separater Terminal-Contract, nicht Teil des laufenden Spiels | Ja — passt exakt zu unserem Phasenauftrag (`mutual_settle`/`oracle_settle` als Terminal-Übergänge aus `MatchEscrow`) | gering | 1:1 übernehmbar als Architekturprinzip |
| Owner-Signaturen (`blake2b(pubkey)==owner` + `checkSig`) | Pubkey nie im State, nur sein Hash; Signatur als Witness-Argument | Ja, direkt übernehmbar für Spieler-Autorisierung | gering | 1:1 übernehmbar |
| Timeout-Handling (`this.ageDaa >= move_timeout`) | Relatives UTXO-Alter als Timeout-Bedingung, permissionless auslösbar („Worker timeout is permissionless") | Ja — direktes Vorbild für `refund_timeout` | gering | 1:1 übernehmbar; `move_timeout` als Konstruktor-Parameter wie bei Chess |
| Terminal-Transitionen (`retire`, mux→Settle bei `status!=LIVE`) | Klar getrennte, nicht umkehrbare Endzustände | Ja | gering | 1:1 übernehmbar |

**Wesentlicher Unterschied Chess vs. KaspaBattle:** Chess-Ergebnisse sind **vollständig on-chain deterministisch** (Zugregeln im Contract selbst geprüft) — es gibt keinen Oracle-Bedarf. KaspaBattle-Matches werden extern auf FACEIT entschieden; das Analogon zu `ChessSettle` (das ohne externen Input direkt aus dem Spielzustand auszahlt) reicht bei uns **nicht** — wir brauchen zusätzlich einen Attestations-/Signatur-Nachweis eines zugelassenen Oracles (Phase 5), wofür Chess keine unmittelbare Vorlage liefert. Dieser Baustein muss in Gate 2 eigenständig entworfen werden (`OpCheckSigFromStack` aus KIP-17 ist der wahrscheinliche Baustein: Oracle signiert eine Attestations-Nachricht, Contract verifiziert sie gegen einen fest hinterlegten Oracle-Pubkey-Hash, ohne dass der Oracle selbst einen Covenant-Input beisteuern muss).

---

## 7. Contract-State / Zustandsdiagramm / Entry-Points (Phase 4) — GATE 2 ENTWURF

**Status: Entwurf zur Freigabe. Noch nicht kompiliert, noch nicht gegen den Interpreter getestet — das ist Gate 3.** Alles unten ist Design-Absicht in SilverScript-naher Pseudo-Syntax, keine verifizierte `.sil`-Datei. Jede verwendete Primitive (`checkSig`, `checkMsgSig`, `blake2b`, `OpAuthOutput*`, Byte-Konkatenation `+`) ist gegen den tatsächlichen SilverScript-Quellcode geprüft (siehe Abschnitt 5.4 und die Fußnoten unten); die exakte P2PK-Skript-Byte-Struktur (Abschnitt 7.3) ist als offene Verifikationsaufgabe für Gate 3 markiert.

### 7.1 Scope-Entscheidung: ein einzelnes Covenant-UTXO, keine Player-Accounts

Die Chess-Referenz (Abschnitt 6) trägt ihr Sicherheitsmodell für Auszahlungs-Ziele über **langlebige `Player`-Covenants** (Auszahlung = Fortsetzung eines bestehenden Player-UTXOs mit unverändertem `owner`-Feld, nie eine direkte Adress-Prüfung). Das ist für uns **nicht direkt übernehmbar**: Player-Ratings/Accounts sind laut Auftrag ausdrücklich außerhalb des MVP-Scopes. Ohne Player-Accounts muss `MatchEscrow` das Auszahlungsziel **selbst** verifizieren — sonst wäre "Output substitution" (Phase 10) trivial ausnutzbar. Deshalb: **jede Entry, die Werte an eine der beiden Parteien auszahlt, prüft `tx.outputs[idx].scriptPubKey` explizit** gegen ein aus dem committeten Owner-Pubkey-Hash abgeleitetes Standard-P2PK-Skript (Abschnitt 7.3). Das ist eine bewusste, begründete Abweichung vom Chess-Vorbild, kein Versehen.

### 7.2 State

```
contract MatchEscrow(
    int      init_contract_version,
    byte[32] init_network_domain,        // blake2b("KASPABATTLE_MATCH_V1" + network_id_string) — Verteidigung in der Tiefe;
                                          // die eigentliche Netzwerk-Bindung kommt aus dem kompilierten Template selbst
                                          // (unterschiedliches Netzwerk -> anderer Bytecode -> anderer template_hash)
    byte[16] init_match_id,              // rohe UUID-Bytes, nicht als String
    byte[32] init_game_id_hash,          // Hash der FACEIT-Spiel-/Lobby-Kennung (variable Länge extern gehasht)
    byte[32] init_player_a,              // blake2b(x-only pubkey) — wie Chess' `owner`
    byte[32] init_player_b,              // Sentinel 0x00..00 = "noch nicht beigetreten"
    int      init_stake_sompi,           // Einsatz PRO Spieler (symmetrisch, MVP-Scope)
    int      init_status,                // 0 CREATED, 1 FUNDED (terminal Übergänge s.u. brauchen keinen State mehr)
    byte[32] init_result_oracle_commitment, // blake2b(x-only oracle pubkey)
    int      init_join_timeout_daa,      // relative DAA-Ticks, NICHT absolute Deadline (s. Anmerkung)
    int      init_result_timeout_daa,    // relative DAA-Ticks ab FUNDED
    int      init_fee_bps,               // Plattform-Fee in Basispunkten (0–10000)
    byte[32] init_treasury_commitment    // blake2b(Treasury-Payout-Skript) — analog zu player_a/b
) { ... }
```

**Direkt gespeichert vs. nur Hash-committed:**

| Feld | Speicherung | Begründung |
|---|---|---|
| `contract_version`, `stake_sompi`, `status`, `fee_bps`, Timeouts | direkt (int, klein) | für jede Entry-Prüfung nötig, keine Vertraulichkeit |
| `network_domain` | direkt, aber redundant zur eigentlichen Bindung über den Template-Hash | Verteidigung in der Tiefe, siehe 7.1-Kommentar |
| `match_id` | direkt, `byte[16]` (rohe UUID) | wird 1:1 für Attestation-Digest gebraucht, keine variable Länge |
| `game_id_hash` | **nur Hash** (`byte[32]`), nicht die rohe FACEIT-ID | FACEIT-IDs sind variabel lang und laut Issue #168 wären dynamische Byte-Arrays als State-Feld ohnehin nicht unterstützt |
| `player_a` / `player_b` | **nur Hash** (`blake2b(pubkey)`), nie der rohe Pubkey | Pubkey kommt bei jeder Aktion als Witness-Argument, exakt wie bei Chess (`owner`) |
| `result_oracle_commitment` | **nur Hash** (`blake2b(pubkey)`) | ein einzelner fest committeter Oracle je Match — keine dynamische Oracle-Governance (Auftragsvorgabe) |
| `treasury_commitment` | **nur Hash** (`blake2b(scriptPubKey)`) | gleiches Muster wie Spieler-Ziele — Treasury-Adresse ist bei Contract-Erstellung fix |

**Kein `nonce`-State-Feld im Contract selbst.** Kaspas UTXO-Modell macht jede Transition ohnehin einmalig (das UTXO ist nach dem Spend weg) — Replay der *eigenen* Transitionen ist strukturell ausgeschlossen. `nonce` gehört ausschließlich in die Attestation (Abschnitt 8): er macht die *off-chain* signierte Nachricht eindeutig, falls der Oracle vor der endgültigen On-Chain-Einreichung mehrmals signiert (z. B. nach einer Korrektur).

**Zu `join_deadline`/`result_deadline` (Auftragsbenennung) → `*_timeout_daa` (Umsetzung):** Die einzige im echten Chess-Code verifizierte Timeout-Primitive ist `this.ageDaa >= move_timeout` — ein **relativer** Vergleich gegen das Alter des ausgegebenen UTXO, keine absolute Deadline. Eine absolute Deadline würde eine andere, in den gelesenen Quellen nicht belegte Primitive brauchen. Wir übernehmen bewusst nur das verifizierte Muster: `join_timeout_daa`/`result_timeout_daa` sind **Dauern**, keine Zeitstempel.

### 7.3 Auszahlungsziel-Verifikation (für alle wertbewegenden Entries)

Gemeinsamer Baustein (kein eigener Entry, sondern wiederverwendete Prüf-Logik):

```
// pk ist Witness-Argument, geprüft gegen das committete Feld:
require(blake2b(byte[](pk)) == player_a /* oder player_b, result_oracle_commitment, treasury */);
// Zieladresse wird NICHT separat committed, sondern deterministisch aus pk abgeleitet:
byte[] expected_spk = <Standard-P2PK-scriptPubKey-Bytes für pk>;
require(tx.outputs[idx].scriptPubKey == expected_spk);
```

**Verifiziert gegen `kaspa-txscript` @ `a41a333b…` (`src/standard.rs`, `src/opcodes/mod.rs`):** Kaspas Standard-P2PK-`scriptPubKey` ist `pay_to_pub_key(pubkey) = OpData32 (0x20) || <32-Byte x-only Pubkey> || OpCheckSig (0xac)` — 34 Bytes, exakt wie Bitcoins klassisches Pay-to-Pubkey-Muster. In SilverScript entspricht das:
```
byte[] expected_spk = byte[1](0x20) + byte[](pk) + byte[1](0xac);
require(tx.outputs[idx].scriptPubKey == expected_spk);
```
`byte[1](0x20)`-Literalsyntax und `byte[](pk)`-Cast sind direkt aus dem realen Chess-Code übernommen (Abschnitt 5.4/6). Verbleibt für Gate 3: das tatsächliche Kompilieren dieses Ausdrucks gegen den echten `silverc`-Compiler bestätigen (Typinferenz von `byte[1](...)`-Konkatenation zu `byte[]` ist im Chess-Code nur für andere Feldbreiten belegt, nicht exakt in dieser 34-Byte-Kombination).

### 7.4 Entry-Points

**`join(sig b_sig, pubkey b_pk)`** — Spieler B tritt bei und zahlt seinen Einsatz ein.
- `require(contract_version == CURRENT_VERSION)`
- `require(status == CREATED)`
- `byte[32] b_owner = blake2b(byte[](b_pk)); require(b_owner != player_a)` — verhindert, dass A sein eigenes Match "annimmt" (Self-Dealing/Griefing, siehe Threat Model)
- `require(checkSig(b_sig, b_pk))`
- `require(OpAuthOutputCount(this.activeInputIndex) == 1)`
- `output_idx = OpAuthOutputIdx(this.activeInputIndex, 0)`
- `require(tx.outputs[output_idx].value == tx.inputs[this.activeInputIndex].value + stake_sompi)` — B's Einsatz muss atomar in **derselben** TX über ein zusätzliches Plain-Input dazukommen; Kaspas TX-Bilanzzwang (`Σinputs == Σoutputs + fee`) erzwingt das strukturell, ohne dass der Contract B's Plain-Input explizit anfassen muss
- neuer State: `player_b = b_owner`, `status = FUNDED`, alle anderen Felder unverändert
- `validateOutputState(output_idx, next_state)` — reine Selbstfortsetzung, **kein** fremdes `readInputStateWithTemplate` in derselben Entry → unkritisch bzgl. Issue #252 (Abschnitt 5.5)

**`cancel_unjoined(sig a_sig, pubkey a_pk)`** — A zieht sein Match zurück, solange niemand beigetreten ist.
- `require(contract_version == CURRENT_VERSION)`
- `require(status == CREATED)`
- `require(blake2b(byte[](a_pk)) == player_a)`
- `require(checkSig(a_sig, a_pk))`
- `require(OpAuthOutputCount(this.activeInputIndex) == 0)` — **wie Chess' `Player::retire`**: sobald die alleinige Eigentümer-Signatur bewiesen ist und niemand sonst beteiligt ist, braucht der Contract das Ziel nicht zu erzwingen — A darf frei über sein eigenes, noch ungeteiltes Geld verfügen. Kein `validateOutputState`/Zielprüfung nötig, da einparteiisch.

**`mutual_settle(sig a_sig, pubkey a_pk, sig b_sig, pubkey b_pk, int a_amount_sompi, int b_amount_sompi)`** — beide Parteien einigen sich auf eine beliebige Aufteilung (z. B. einvernehmlicher Abbruch, Split), ohne Oracle.
- `require(contract_version == CURRENT_VERSION)`
- `require(status == FUNDED)`
- `require(blake2b(byte[](a_pk)) == player_a); require(checkSig(a_sig, a_pk))`
- `require(blake2b(byte[](b_pk)) == player_b); require(checkSig(b_sig, b_pk))`
- `require(a_amount_sompi >= 0 && b_amount_sompi >= 0)`
- `require(a_amount_sompi + b_amount_sompi == 2 * stake_sompi)` — **keine** Fee bei einvernehmlicher Einigung (bewusste Design-Entscheidung: die Plattform-Fee ist an das Oracle-Settlement gekoppelt, nicht an private Einigungen — siehe Threat Model „Fee-Umgehung ist hier kein Diebstahl, sondern von beiden Parteien gewollt")
- `require(OpAuthOutputCount(this.activeInputIndex) == 2)`
- zwei Outputs, je gegen `player_a`/`player_b` verifiziert wie in 7.3, mit `tx.outputs[idx].value == a_amount_sompi` bzw. `b_amount_sompi`
- Kein `validateOutputState` (terminal, keine Fortsetzung) — beide Signaturen sichern bereits die gesamte Transaktion inkl. Beträge über den nativen Sighash-Mechanismus; keine zusätzliche Attestation nötig.

**`oracle_settle(datasig oracle_sig, pubkey oracle_pk, int winner_selector, byte[32] result_hash, int observed_at, byte[32] nonce)`** — permissionless, jeder (typischerweise unser Backend oder der Gewinner selbst) kann die TX einreichen, sobald eine gültige Oracle-Attestation vorliegt. **Kein Spieler muss online sein oder signieren** — Verbesserung gegenüber dem heutigen PSKT-Fluss, der die Live-Signatur des Gewinners braucht.
- `require(contract_version == CURRENT_VERSION)`
- `require(status == FUNDED)`
- `require(winner_selector == 0 || winner_selector == 1)`
- `require(blake2b(byte[](oracle_pk)) == result_oracle_commitment)`
- Digest **aus den eigenen State-Feldern plus den Witness-Feldern** rekonstruieren (siehe Abschnitt 8 für das exakte Format) — **nicht** aus einem separat übergebenen, potenziell manipulierbaren Digest-Wert:
  `byte[] msg = byte[]("KASPABATTLE_RESULT_V1") + network_domain + contract_version_bytes + match_id + game_id_hash + winner_selector_bytes + result_hash + observed_at_bytes + nonce; byte[32] digest = blake2b(msg);`
- `require(checkMsgSig(oracle_sig, digest, oracle_pk))` — verifiziert über `OpCheckSigFromStack` (KIP-17), **nicht** die transaktionsgebundene `checkSig`
- Gewinner-Pubkey-Hash `winner_owner = (winner_selector == 0) ? player_a : player_b`, Verlierer entsprechend umgekehrt
- Pot: `total = 2 * stake_sompi`; `fee = total * fee_bps / 10000`; `payout = total - fee`
- `require(OpAuthOutputCount(this.activeInputIndex) == 2)` — ein Output an den Gewinner, ein Output an die Treasury (fee); bei `fee_bps == 0` könnte man auf 1 Output reduzieren, MVP hält es für Gate 3 einfach bei fest 2
- Gewinner-Output gegen `winner_owner` verifiziert wie 7.3, Wert `== payout`
- Treasury-Output gegen `treasury_commitment` verifiziert (analog 7.3, aber gegen ein committetes Skript statt live abgeleitetem Pubkey-Skript — Treasury ist keine Pubkey-Identität, sondern eine fest hinterlegte Ziel-Skript-Hash), Wert `== fee`

**`refund_timeout(pubkey caller_pk)`** — permissionless Notausstieg, wenn FUNDED zu lange ohne Settlement bleibt (Oracle antwortet nie, FACEIT-Match wird nie ausgetragen, etc.).
- `require(contract_version == CURRENT_VERSION)`
- `require(status == FUNDED)`
- `require(this.ageDaa >= result_timeout_daa)` — **wie Chess' `timeout`**: rein UTXO-Alter-basiert, kein externer Zeitstempel nötig
- `require(OpAuthOutputCount(this.activeInputIndex) == 2)`
- je ein Output à `stake_sompi` an `player_a` und `player_b`, je gegen 7.3 verifiziert — **keine Fee bei Refund** (niemand hat "gewonnen", die Plattform hat nichts geleistet)
- `caller_pk` wird **nicht** geprüft (Aufruf ist bewusst permissionless, jeder darf den Refund auslösen — analog Chess' Kommentar „Worker timeout is permissionless")

### 7.5 Zustandsdiagramm

```
CREATED ──join──────────────▶ FUNDED ──oracle_settle────▶ (terminal: Gewinner-Payout + Fee)
   │                             │
   └──cancel_unjoined──▶         ├──mutual_settle─────────▶ (terminal: vereinbarte Aufteilung)
     (terminal: A erhält         │
      seinen Einsatz zurück)     └──refund_timeout────────▶ (terminal: 50/50 Rückerstattung)
```

Kein Zustand ist nach einem terminalen Übergang mehr vorhanden — alle vier Endpfade konsumieren das Covenant-UTXO vollständig und erzeugen **keine** Fortsetzung desselben Contract-Typs (anders als Chess' `mux`↔`settle`-Zyklus, der für ein 1v1-Match mit genau einem Ergebnis nicht nötig ist).

### 7.6 Gate-3-Fund: `this.ageDaa` unterstützt nur `>=`

Beim ersten echten Kompilierversuch (siehe 7.7) stellte sich heraus: `this.ageDaa` ist **kein** normaler vergleichbarer Ausdruck, sondern eine eigene Grammatik-Sonderform (`TxVar`, lowert zu `Statement::RequireAgeDaa`) — **nur** `require(this.ageDaa >= <ausdruck>)` ist zulässig, ein `<`, `>`, `&&`-Verknüpfung o. ä. ist ein Parse-Fehler. Damit lässt sich **kein** "jünger als X"-Check ausdrücken. Konsequenz: `join_timeout_daa` kann in `join` **nicht** als "Beitritt nur vor Ablauf" durchgesetzt werden, wie ursprünglich in 7.4 vorgesehen. Das Feld bleibt vorerst unenforced/reserviert (im State abgelegt, aber ohne aktive Prüfung) — die einzige tatsächlich nutzbare Timeout-Richtung ist "mindestens X alt", passend zu `refund_timeout`. Das ist keine Design-Lücke, sondern eine durch den echten Compiler aufgedeckte Grenze, die vor der Implementierung (und nicht erst auf Testnet) gefunden wurde — genau der Zweck von Gate 3.

### 7.7 Gate-3-Fortschritt: erste erfolgreiche Kompilierung (verifiziert)

`contracts/silverscript/match_escrow.sil` wurde gegen den echten `silverc` (SilverScript `v1.0.0` @ `3ed9733…`, rusty-kaspa @ `a41a333b…`) kompiliert — **erfolgreich, keine Fehler**, nach den beiden oben dokumentierten Korrekturen (int-Encoding, `this.ageDaa`-Grenze). Artefakte: `contracts/artifacts/match_escrow.abi.json` (ABI-Schema-Version 1) und `contracts/artifacts/match_escrow.manifest.json`.

| Entry | Dispatch-Tag (hex) |
|---|---|
| `join` | `8bb176b9` |
| `cancel_unjoined` | `f7144901` |
| `mutual_settle` | `bb8a5e32` |
| `oracle_settle` | `414681ad` |
| `refund_timeout` | `54b3d885` |

Template-Hash (Stand vor 7.8-Fixes): `8d96afea1e22818ec3f9b6781e8e41a8721f87b8245bfabe069d5701acb5d357`. Kompilierter Bytecode: 1213 Bytes — deutlich unter jedem bekannten Größenlimit.

### 7.8 Gate-3-Fortschritt: Interpreter-Tests — 10/10 grün (zwei weitere reale Bugs gefunden und gefixt)

Ausführung gegen den **echten Interpreter** (`kaspa_txscript::TxScriptEngine`, `covenants_enabled: true`, reale Schnorr-Signaturen, reale Sighashes) mit einer eigenen Rust-Testsuite nach exakt dem Muster von SilverScripts eigenem `chess_apps_tests.rs` — siehe `contracts/silverscript/tests/interpreter_tests.rs` + `README.md` dort für Details und Ausführungsanleitung (läuft **nicht** in diesem Repo, da `kaspabattle`'s eigener Cargo-Workspace mit dem alten 0.15er-rusty-kaspa kollidiert — erfordert eine separate Checkout gegen den gepinnten SilverScript-Rev).

**Ergebnis: 10/10 Tests grün** für `join`, `cancel_unjoined`, `mutual_settle`, `oracle_settle` (happy path + jeweils 1-2 gezielte Angriffs-/Fehlerfälle, u. a. „Output substitution" aus dem Threat Model in Abschnitt 10 direkt am Interpreter widerlegt). `refund_timeout` ist **nicht** interpreter-getestet — `this.ageDaa` braucht einen echten/Simnet-Konsens-Kontext, den der leichtgewichtige Test-Harness nicht liefert (SilverScripts eigene Testsuite testet `this.ageDaa`-Entries aus demselben Grund auch nicht auf diesem Weg).

Dabei zwei weitere reale, vorher nicht bekannte Bugs gefunden (beide **vor** diesem Stand bereits im Contract-Quelltext gefixt, nicht offen gelassen):

1. **`tx.outputs[idx].scriptPubKey` enthält ein 2-Byte-Big-Endian-Versionspräfix** (`ScriptPublicKeyVersion = u16`), nicht nur die rohen Skript-Bytes — verifiziert gegen `kaspa_txscript`s `SpkEncoding::to_bytes()` am gepinnten Rev (`self.version.to_be_bytes().chain(script bytes)`). Die in 7.3 beschriebene P2PK-Konstruktion war dadurch anfangs **falsch** (34 statt 36 Bytes) und ließ jeden scriptPubKey-Vergleich fehlschlagen. Fix: `byte[2](0x0000) + byte[1](0x20) + byte[](pk) + byte[1](0xac)` in allen betroffenen Entries (`mutual_settle`, `oracle_settle`, `refund_timeout`).
2. **`covenant_id` auf einem Output ist kein frei wählbares Tag.** Verifiziert gegen `kaspa_txscript::covenants::CovenantsContext::from_tx`: Stimmt ein Output-`covenant_id` mit dem `covenant_id` des autorisierenden (gespenten) Inputs überein, zählt er als **Fortsetzung** (wird von `OpAuthOutputCount`/`OpAuthOutputIdx` gezählt) — unabhängig davon, ob sein `scriptPubKey` selbst ein Covenant-Skript ist oder ein simpler P2PK-Payout. Weicht er ab, gilt er als **Genesis** (neuer Covenant) und muss einen protokoll-abgeleiteten Wert tragen (aus dem gespenten Outpoint berechnet); ein frei gewählter Wert wird mit `CovenantsError::WrongGenesisCovenantId` abgelehnt und **taucht in keinem Auth-Output-Kontext auf**. Für terminale Payout-Outputs (`mutual_settle`/`oracle_settle`/`refund_timeout`) muss deshalb dieselbe `covenant_id` wie das gespente Input-UTXO verwendet werden — eine erste Testvariante mit je einer eigenen ID pro Output schlug genau deshalb fehl.

**Aktualisierter Template-Hash nach beiden Fixes:** `53acf11fd0d084048188db58b71277eb5eca2373a2a9e0068569caf6c2e8cc7d` (`contracts/artifacts/match_escrow.abi.json`/`match_escrow.manifest.json` sind auf diesem Stand).

**Noch offen (verbleibender Gate-3-Umfang):** `refund_timeout` interpreter-/simnet-testen; Rust-/TypeScript-Attestation-Referenzimplementierung mit Testvektoren (8.3); danach erst eine echte kleine Testnet-10-Transaktion (Gate 3 selbst, Freigabe nötig).

---

## 8. Result-Attestation (Phase 5)

### 8.1 Kanonisches Format `KASPABATTLE_RESULT_V1`

Deterministische Byte-Konkatenation (keine JSON-Serialisierung — vermeidet jede Feldreihenfolge-/Whitespace-Ambiguität):

```
msg = "KASPABATTLE_RESULT_V1"      (22 ASCII-Bytes, Domain-Separator)
    || network_domain              (32 Bytes — blake2b("KASPABATTLE_MATCH_V1" + network_id_string), MUSS mit dem
                                     Contract-State-Feld übereinstimmen, siehe 7.2)
    || contract_version             (4 Bytes — SilverScripts `int as byte[4]`-Encoding, s.u.)
    || match_id                     (16 Bytes, rohe UUID — aus dem Contract-State, nicht separat übergeben)
    || game_id_hash                 (32 Bytes — aus dem Contract-State)
    || winner_selector               (1 Byte: 0x00 = player_a, 0x01 = player_b)
    || result_hash                  (32 Bytes — off-chain Commitment auf die vollständigen FACEIT-Match-Details:
                                     blake2b(faceit_match_id || score_string || …); Detailformat ist Backend-intern,
                                     nur der Hash geht on-chain)
    || observed_at                  (8 Bytes — `int as byte[8]`-Encoding, s.u. — DAA-Score oder Unix-Zeit zum
                                     Beobachtungszeitpunkt des Oracles; rein informativ, nicht konsensrelevant
                                     außer als Teil des signierten Digests)
    || nonce                        (32 Bytes — vom Oracle zufällig gewählt, macht jede Signatur eindeutig, falls
                                     vor der On-Chain-Einreichung mehrfach signiert wird, z. B. nach Korrektur)

digest = blake2b(msg)               // 32 Bytes — DAS wird signiert, nicht `msg` selbst (Anforderung von
                                     // `checkMsgSig`/OpCheckSigFromStack: exakt byte[32])
signature = SchnorrSign(oracle_privkey, digest)   // kompatibel mit OpCheckSigFromStack (KIP-17, 0xd7)
```

**Int-Encoding, präzise verifiziert (nicht angenommen):** SilverScripts `int as byte[N]`-Cast kompiliert zu `OpNum2Bin` (`kaspa-txscript` @ gepinntem Rev, `opcodes/mod.rs`), was wiederum `serialize_i64(value, Some(N))` aufruft. Das ist **Bitcoins klassisches `CScriptNum`-Little-Endian-Encoding**: Little-Endian-Bytes des Betrags, rechts mit Nullen auf `N` Bytes aufgefüllt, und falls negativ, wird Bit `0x80` im **letzten** Byte gesetzt. **Nicht** big-endian, wie eine erste Entwurfsfassung dieses Dokuments fälschlich annahm — das war eine unverifizierte Annahme und wurde hier korrigiert, bevor Code entstand. Für unsere ausschließlich nicht-negativen Werte (`contract_version`, `winner_selector`, `observed_at`) ist das äquivalent zu "Little-Endian, mit Nullen aufgefüllt", **aber** die Rust-/TypeScript-Referenzimplementierung (8.3) muss exakt `serialize_i64` nachbauen (nicht naives `to_le_bytes()`), um Byte-für-Byte-Übereinstimmung mit dem kompilierten Contract zu garantieren — insbesondere die Sonderbehandlung, wenn das natürliche Minimal-Encoding bereits Bit `0x80` gesetzt hätte.

**Netzwerk-Bindung:** `network_domain` verhindert Replay einer testnet-10-Attestation auf einem späteren Mainnet-Contract (unterschiedliche `network_id_string` → anderer Digest → Signatur passt nicht). Zusätzlich verhindert der unterschiedliche kompilierte Bytecode/Template-Hash pro Netzwerk (Abschnitt 5.1) ohnehin, dass ein testnet-10-Contract auf Mainnet überhaupt als "derselbe" Contract-Typ akzeptiert würde.

**Keine serverseitige Einschleusung beliebiger Empfänger-Outputs:** Der Digest enthält **keine** Empfänger-Adresse/-Skript. Das Empfängerziel wird ausschließlich im Contract selbst aus `player_a`/`player_b`/`treasury_commitment` (State, bei Contract-Erstellung fixiert) plus `winner_selector` (im Digest, also vom Oracle attestiert) abgeleitet — ein kompromittiertes Backend, das die Transaktion baut, kann `winner_selector` nicht ändern, ohne die Oracle-Signatur ungültig zu machen, und kann die Empfänger-Skripte generell nicht frei wählen (siehe 7.3).

### 8.2 Zwei Settlement-Pfade

1. **`mutual_settle`** — beide Spieler signieren live die Transaktion selbst (`checkSig`, transaktionsgebunden). Kein Attestation-Objekt nötig; die native Sighash-Bindung deckt Beträge und Empfänger bereits ab.
2. **`oracle_settle`** — ein einzelner autorisierter Testnet-Oracle signiert eine `KASPABATTLE_RESULT_V1`-Attestation off-chain (`checkMsgSig`/`OpCheckSigFromStack`); die Transaktion kann von *jedem* eingereicht werden, sobald diese Signatur vorliegt.

**Wichtige Klarstellung (Auftragsvorgabe):** SilverScript dezentralisiert **Escrow und Auszahlung** — es macht **nicht** die externe FACEIT-Datenquelle dezentral oder vertrauenslos. Der Oracle liest weiterhin von FACEITs API (siehe `docs/03-SECURITY.md` „Oracle (FACEIT results): Trusted"); was sich ändert, ist dass der Oracle **nur noch attestiert**, aber nicht mehr selbst die Auszahlungs-TX signiert/broadcastet — die Auszahlung ist kryptographisch an die Attestation gebunden, nicht an eine serverseitig kontrollierte Signierhandlung.

### 8.3 Testvektoren (Rust + TypeScript)

**Noch nicht erstellt — Teil der Implementierung nach Gate 2.** Geplanter Ort: `kaspabattle/battle-silverscript/src/attestation.rs` (Rust-Referenzimplementierung + `#[cfg(test)]`-Vektoren) und ein TypeScript-Äquivalent unter `battle-frontend/src/kaspa/` oder einem neuen `attestation.ts`, das exakt dieselben Testvektoren (feste Eingaben → fester `digest`-Hex-Wert) prüft, um Rust/TS-Implementierungsdrift auszuschließen.

---

## 9. Oracle-/Wallet-/RPC-Modell (Phase 5/7/8) — Kurzfassung für Gate 2

- **Oracle-Modell:** ein fest committeter Oracle-Pubkey-Hash pro Match (`result_oracle_commitment`), identisch zum bestehenden `ORACLE_PRIVATE_KEY`-Muster (ein Plattform-Oracle-Key, kein dynamisches Multi-Oracle-Set — Auftragsvorgabe „keine dynamische Oracle-Governance"). Der bestehende `OracleService`/FACEIT-Watcher (`battle-kaspa/src/oracle.rs`) ändert sich in seiner FACEIT-Anbindung **nicht** — er bekommt nur einen neuen Ausgabe-Pfad: statt eine PSKT zu bauen und zu signieren, erzeugt er eine `KASPABATTLE_RESULT_V1`-Attestation und signiert **den Digest**, nicht mehr eine Transaktion.
- **Wallet-Modell:** noch nicht abschließend geklärt, siehe offene Fragen unten. Das bestehende `battle-frontend/src/kaspa/wallet.ts`-Signiermodell (`pending.sign(privateKeys)`) deckt transaktionsgebundene Signaturen (`mutual_settle`) potenziell ab, aber `join`/`cancel_unjoined` brauchen ebenfalls nur Standard-Transaktionssignaturen — **kein** neues clientseitiges Signaturschema nötig für die Spieler-Seite, sofern das vendierte `kaspa-wasm` covenant-fähige Transaktionen überhaupt bauen kann (unverifiziert, s. u.).
- **Offene Fragen für die Implementierungsphase (nicht Gate 2, aber hier festgehalten, damit sie nicht verloren gehen):**
  1. Kann das vendierte `kaspa-wasm` (v1.1.0-rc.3, keine Covenant-Symbole gefunden) covenant-tragende Transaktionen überhaupt bauen/signieren, oder brauchen wir eine neuere `kaspa-wasm`-Version bzw. serverseitigen TX-Bau (Browser bekommt nur eine unsignierte TX zum Signieren, wie im Auftrag als Zielbild beschrieben)?
  2. Das exakte P2PK-`scriptPubKey`-Byte-Layout (Abschnitt 7.3) muss aus dem gepinnten `kaspa-txscript`-Rev übernommen werden.
  3. Compute-Budget-Kalibrierung (Issue #243) für jede Entry-Funktion — kein Schätzer verfügbar, nur Trial-and-Error gegen einen echten testnet-10-Node.
- **RPC-Modell:** unverändert zum bestehenden Ansatz (Resolver-first, optionale explizite `KASPA_NODE_URL`, kein eigener Node) — muss aber auf **testnet-10** zeigen (Abschnitt 5.2), nicht testnet-12 wie der aktuelle Backend-Default.

---

## 10. Threat Model (Phase 10, Gate-2-Teil)

**Kerninvariante (Auftragsvorgabe):** *Auch wenn Backend oder Datenbank vollständig kompromittiert sind, darf keine Auszahlung an einen nicht durch den Contract autorisierten Empfänger möglich sein.* Das Design in Abschnitt 7 ist genau darauf ausgelegt: jeder wertbewegende Output wird gegen ein bei Contract-Erstellung fixiertes Commitment geprüft (7.3), nie gegen einen vom Transaktions-Ersteller frei wählbaren Wert.

| Bedrohung | Betroffene Entry(s) | Mitigation im Design | Restrisiko |
|---|---|---|---|
| **Contract transition substitution** (falscher Entry-Typ akzeptiert) | alle | Dispatch-Tag ist `blake3(entry_name + Typsignatur)` (5.3), Kollisionen sind Compile-Fehler; jede Entry prüft `status` explizit | gering |
| **Output substitution** (Geld an falsche Adresse) | `join`, `mutual_settle`, `oracle_settle`, `refund_timeout` | explizite `scriptPubKey`-Prüfung gegen committete Pubkey-Hashes (7.3) — **bewusste Abweichung von Chess**, s. 7.1 | **gering für `mutual_settle`/`oracle_settle`** — am echten Interpreter widerlegt (7.8, `oracle_settle_rejects_redirected_winner_output`); für `refund_timeout` **noch nicht interpreter-getestet** (this.ageDaa-Limitierung, 7.8) |
| **Double payout** (UTXO zweimal ausgegeben) | alle terminalen | strukturell durch Kaspas UTXO-Modell ausgeschlossen (Input ist nach einem Spend weg); kein zusätzlicher Schutz nötig | sehr gering |
| **Fee siphoning** (Fee-Betrag manipuliert) | `oracle_settle` | `fee`/`payout` werden **im Contract** aus `stake_sompi`/`fee_bps` berechnet, nicht vom TX-Ersteller vorgegeben; `require(tx.outputs[idx].value == payout)` exakt | gering |
| **Oracle key compromise** | `oracle_settle` | Schaden ist **pro Match begrenzt** (ein `result_oracle_commitment` pro Contract-Instanz, kein globaler Oracle-Key, der alle Matches gleichzeitig betrifft, sofern man den Oracle-Key rotiert) — aber MVP nutzt vermutlich denselben Oracle-Pubkey für alle Matches (siehe „keine dynamische Oracle-Governance"), also **de facto global** trotz Pro-Match-Commitment | **hoch, wie im bestehenden System auch (unverändert ggü. Multisig-Modell)** — außerhalb des MVP-Scopes lösbar (Oracle-Rotation bräuchte neue Matches mit neuem committetem Key; bestehende FUNDED-Matches bleiben am alten Key hängen bis Settlement/Timeout) |
| **Oracle equivocation** (zwei widersprüchliche Attestationen) | `oracle_settle` | strukturell irrelevant: UTXO ist nach dem ersten gültigen `oracle_settle` weg, eine zweite (widersprüchliche) Attestation hat kein UTXO mehr zum Einlösen | gering, **aber**: wenn zwei widersprüchliche, gültig signierte Attestationen gleichzeitig im Mempool konkurrieren, gewinnt schlicht, wessen TX zuerst bestätigt wird — kein On-Chain-Dispute-Mechanismus im MVP (Scope-Entscheidung, siehe unten) |
| **Replay** (alte gültige Aktion erneut einreichen) | alle | UTXO-Konsum macht jede Transition einmalig; Attestation-`nonce` schützt zusätzlich die *Signatur* vor Wiederverwendung außerhalb ihres UTXO-Kontexts | gering |
| **Cross-network replay** (Testnet-Attestation auf Mainnet) | `oracle_settle` | `network_domain` im Digest + unterschiedlicher Template-Hash pro Netzwerk (5.1/5.2) | gering, sofern 8.1 korrekt implementiert |
| **Timeout race** (Settlement und Timeout gleichzeitig eingereicht) | `oracle_settle` vs. `refund_timeout` | `refund_timeout` erfordert `status == FUNDED` UND `ageDaa >= result_timeout_daa`; ein rechtzeitiges `oracle_settle` konsumiert das UTXO zuerst und macht `refund_timeout` gegenstandslos — normale Mempool-Konkurrenz, kein Extra-Schutz nötig, aber **Backend sollte `oracle_settle` proaktiv vor Ablauf des Timeouts einreichen** | gering, operationell zu beachten |
| **Frontend XSS** | Spieler-seitiges Signieren | unverändert zum bestehenden Risiko (Mnemonic im Zustand-Store, siehe `docs/LEARNINGS.md` Gate-1-Befund) — SilverScript ändert daran nichts, es sei denn Phase 7 führt einen externen Wallet-Adapter ein | unverändert, außerhalb dieses Contract-Designs |
| **Malicious backend** | TX-Bau für alle Entries | Kerninvariante oben — Backend kann TXen bauen, aber nicht deren Gültigkeit gegen den Contract erzwingen | durch Design adressiert, **abhängig von 7.3-Verifikation** |
| **Manipulierte Datenbank** | Restore-Pfade (`restore_escrow_from_row`-Äquivalent) | Anders als beim bestehenden Multisig-Modell (Abschnitt „SEC-MULTISIG-01") braucht `MatchEscrow` **keine** aus der DB rekonstruierbaren Private Keys — die Contract-Identität lebt vollständig on-chain im UTXO/Template, die DB ist nur noch ein Index/Cache. Eine manipulierte DB kann bestenfalls die falsche TX zur falschen Zeit bauen *versuchen*, aber nicht gegen den Contract durchsetzen | **strukturell deutlich sicherer als das bestehende Multisig-Modell** |
| **RPC liveness / falscher Netzwerkmodus** | alle | testnet-12-Fehlkonfiguration (Abschnitt 5.2) würde schlicht keine gültige Verbindung/kein gültiges Netzwerk ergeben, kein Sicherheits-, sondern ein Verfügbarkeitsrisiko | gering, aber **muss vor jeder Implementierung auf testnet-10 umgestellt werden** |
| **Reorg** | frisch bestätigte Deposits/Settlements | wie im bestehenden System: Kaspas hohe BPS-Rate + DAA-Tiefe vor "confirmed"-Markierung abwarten; unverändert zur bestehenden Mitigation in `docs/03-SECURITY.md` | unverändert |
| **ABI mismatch** | Backend-Start | Manifest-Validierung (Phase 6, siehe unten) — Backend verweigert den SilverScript-Modus-Start bei Abweichung, kein stiller Fallback | durch geplantes Manifest-Design adressiert (Implementierungsdetail, nicht mehr Gate 2) |
| **Compiler supply chain** | Build-Zeit | fester Git-Rev-Pin (`a41a333b…`, `v1.0.0`), kein `master`; Cargo.lock committed | durch Vorgehen bereits adressiert |
| **Gestohlene Dev-Keys** | Testnet-Betrieb | Testnet-only, keine echten Werte — Schaden auf Testnet-KAS begrenzt (Auftragsvorgabe „kein Mainnet") | durch Scope begrenzt |
| **Manipulierte Contract-Artefakte** | Backend-Start | Template-Hash-Prüfung gegen Manifest beim Start (Phase 6) | durch geplantes Manifest-Design adressiert |
| **Self-Dealing/Griefing** (A "tritt" seinem eigenen Match bei) | `join` | `require(b_owner != player_a)` (7.4) | gering |

**Bewusst nicht gelöst im MVP (Scope-Entscheidung, nicht vergessen):** Es gibt **keinen** `dispute`-Entry-Point — der Auftrag listet exakt fünf Entries (`join`, `cancel_unjoined`, `mutual_settle`, `oracle_settle`, `refund_timeout`), keinen Dispute-Pfad. Eine falsch attestierte `oracle_settle`-Transition ist im MVP **endgültig**, sobald sie bestätigt ist — anders als das bestehende System, das einen `Disputed`-Status kennt (`docs/03-SECURITY.md`, F-009). Das ist ein bewusster Komplexitäts-Trade-off der Vorgabe, kein Versehen; sollte vor produktivem Einsatz (auch auf Testnet mit echten Nutzern) noch einmal explizit mit Timo abgestimmt werden.

---

## 11. Phase 6 — Modulare Integration (Kurzentwurf)

```rust
// kaspabattle/battle-kaspa/src/settlement.rs (neu) — Abstraktion über beide Escrow-Backends
#[async_trait]
pub trait MatchSettlement {
    async fn create_match(&self, match_id: Uuid, stake_sompi: u64, /* … */) -> Result<...>;
    async fn join_match(&self, match_id: &Uuid, /* … */) -> Result<...>;
    async fn mutual_settle(&self, match_id: &Uuid, /* … */) -> Result<...>;
    async fn oracle_settle(&self, match_id: &Uuid, /* … */) -> Result<...>;
    async fn refund_timeout(&self, match_id: &Uuid) -> Result<...>;
    async fn inspect_state(&self, match_id: &Uuid) -> Result<...>;
}
```

- `LegacyMultisigSettlement` = dünner Wrapper um das bestehende `MultisigEscrowService` (Abschnitt „SEC-MULTISIG-01" bleibt dessen Sicherheitsbasis).
- `SilverScriptSettlement` = neue Implementierung in einem neuen Crate `kaspabattle/battle-silverscript/` (`artifact.rs`, `state.rs`, `attestation.rs`, `builder.rs`, `transitions.rs`, `errors.rs` — wie im Auftrag vorgeschlagen).
- `SETTLEMENT_MODE=legacy_multisig|silverscript_testnet` steuert, welche Implementierung für **neue** Matches verwendet wird; `settlement_mode` und `contract_version`/`template_hash` werden **pro Match** in der DB gespeichert (neue Migration, additiv, siehe `CLAUDE.md`-Regel „nur neue Migrationsdateien"). Bereits finanzierte Multisig-UTXOs werden **nicht** migriert.
- **Manifest** (`contracts/artifacts/match_escrow.manifest.json`): `contract_name`, `contract_version`, `source_hash` (Hash der `.sil`-Datei), `compiler_revision` (`3ed9733…`), `rusty_kaspa_revision` (`a41a333b…`), `abi_schema_version` (aktuell `1`, siehe `SIL_ABI_SCHEMA_VERSION` in Abschnitt 5.3), `template_hash`, `network` (`testnet-10`). Backend-Start im SilverScript-Modus validiert ABI-Schema-Version und Template-Hash gegen dieses Manifest und **verweigert den Start** bei Abweichung — kein stiller Fallback auf einen anderen Bytecode (Auftragsvorgabe, deckt sich mit dem Threat-Model-Eintrag „ABI mismatch" oben).

Implementierung dieses Crates/Manifests ist **nicht** Teil von Gate 2 — das ist der nächste Schritt nach Freigabe.

---

---

## Referenzen

- [kaspanet/silverscript](https://github.com/kaspanet/silverscript) @ `v1.0.0` (`3ed973335b59269293564805cc2c58a14595ec03`)
- [kaspanet/rusty-kaspa](https://github.com/kaspanet/rusty-kaspa) @ `a41a333b08848f41bf737b72592e463a6011b8ac` (SilverScript-Pin)
- [KIP-17 — Covenants and Improved Scripting Capabilities](https://github.com/kaspanet/kips/blob/master/kip-0017.md)
- [docs.kaspa.org/toccata/silverscript](https://docs.kaspa.org/toccata/silverscript)
- SilverScript-Issues: [#256](https://github.com/kaspanet/silverscript/issues/256), [#254](https://github.com/kaspanet/silverscript/issues/254), [#253](https://github.com/kaspanet/silverscript/issues/253), [#252](https://github.com/kaspanet/silverscript/issues/252), [#243](https://github.com/kaspanet/silverscript/issues/243), [#242](https://github.com/kaspanet/silverscript/issues/242), [#241](https://github.com/kaspanet/silverscript/issues/241), [#240](https://github.com/kaspanet/silverscript/issues/240), [#218](https://github.com/kaspanet/silverscript/issues/218), [#168](https://github.com/kaspanet/silverscript/issues/168), [#139](https://github.com/kaspanet/silverscript/issues/139)
