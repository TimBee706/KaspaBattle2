# SilverScript L1 Escrow — Integration Plan

Status: **Phase 0–3 abgeschlossen (Gate 1 freigegeben 2026-09-29). Gate 2 (Contract-Spec + Threat Model) steht noch aus.**
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

## 7. Contract-State / Zustandsdiagramm / Entry-Points (Phase 4)

*Noch offen — folgt nach Freigabe von Gate 2 (Contract-Spezifikation + Threat Model).*

## 8. Result-Attestation (Phase 5)

*Noch offen — folgt mit Gate 2.*

## 9. Oracle-Modell / Wallet-Modell / RPC-Modell ohne eigenen Node (Phase 5/7/8)

*Noch offen — folgt mit Gate 2. Vorläufige Beobachtung: vendiertes `kaspa-wasm` (v1.1.0-rc.3) hat keine erkennbaren Covenant-Symbole; muss vor Phase 7 gegen eine SilverScript-kompatible `kaspa-wasm`-Version geprüft werden.*

## 10. Migrationsplan / Testplan / Risiken / Rollback-Plan / Mainnet-Readiness-Checkliste

*Noch offen — folgt mit Gate 2 bzw. Gate 3/4.*

---

## Referenzen

- [kaspanet/silverscript](https://github.com/kaspanet/silverscript) @ `v1.0.0` (`3ed973335b59269293564805cc2c58a14595ec03`)
- [kaspanet/rusty-kaspa](https://github.com/kaspanet/rusty-kaspa) @ `a41a333b08848f41bf737b72592e463a6011b8ac` (SilverScript-Pin)
- [KIP-17 — Covenants and Improved Scripting Capabilities](https://github.com/kaspanet/kips/blob/master/kip-0017.md)
- [docs.kaspa.org/toccata/silverscript](https://docs.kaspa.org/toccata/silverscript)
- SilverScript-Issues: [#256](https://github.com/kaspanet/silverscript/issues/256), [#254](https://github.com/kaspanet/silverscript/issues/254), [#253](https://github.com/kaspanet/silverscript/issues/253), [#252](https://github.com/kaspanet/silverscript/issues/252), [#243](https://github.com/kaspanet/silverscript/issues/243), [#242](https://github.com/kaspanet/silverscript/issues/242), [#241](https://github.com/kaspanet/silverscript/issues/241), [#240](https://github.com/kaspanet/silverscript/issues/240), [#218](https://github.com/kaspanet/silverscript/issues/218), [#168](https://github.com/kaspanet/silverscript/issues/168), [#139](https://github.com/kaspanet/silverscript/issues/139)
