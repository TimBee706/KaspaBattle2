# KaspaBattle Lobby- und Match-Flow (v0.2 Update)

Dieses Dokument beschreibt den **UX- und State-Flow** für Lobbies, Einzahlungen (Escrow) und Matchstart in KaspaBattle.[file:1]

**Updates v0.2:**
- FaceID ist **optional** (nicht mandatory, kein Blocker).
- **Kein Timeout** für WAITING_FOR_PLAYER / PENDING_DEPOSITS (Testmodus).
- Spieler B kann ebenfalls Lobbys erstellen (symmetrisch).[file:1][file:15]

---

## 1. Zielbild

Eine Lobby ist zunächst offen und auf „Warten auf zweiten Spieler“.  
Ein zweiter Spieler kann joinen, solange der Status „WAITING_FOR_PLAYER" ist.  
Beide Spieler müssen ihren Wager-Betrag auf das Escrow-Wallet einzahlen, bevor der Status auf „LOCKED / IN_GAME" wechselt.

FaceID ist optionaler Sicherheitslayer, kein Blocker für Matchstart.

---

## 2. Zustandsmodell der Lobby

### 2.1 Lobby-States (Frontend / Off-Chain)

[file:1][file:15]

- `DRAFT` – Lokal vorbereitet, nicht veröffentlicht.  
- `WAITING_FOR_PLAYER` – Spieler A hat Lobby veröffentlicht, B fehlt. **Kein Timeout**.  
- `PENDING_DEPOSITS` – Beide Spieler zugeordnet, mind. 1 Deposit fehlt. **Kein Timeout**.  
- `READY_TO_LOCK` – Beide Deposits bestätigt. FaceID optional.  
- `LOCKED / IN_GAME` – Match gestartet, Escrow gesperrt.  
- `RESOLVING` – Ergebnisabfrage läuft.  
- `RESOLVED` – Gewinner ausgezahlt.  
- `CANCELLED` – Manueller Abbruch (kein Auto-Timeout).

### 2.2 On-Chain States (MatchEscrow)

[file:1]

| UI-State            | Contract-State |
|---------------------|----------------|
| WAITING_FOR_PLAYER  | OPEN           |
| PENDING_DEPOSITS    | OPEN/FUNDED    |
| READY_TO_LOCK       | FUNDED         |
| LOCKED / IN_GAME    | LOCKED         |

---

## 3. Ablauf: Lobby bis Match-Start

### 3.1 Lobby erstellen (Spieler A oder B)

**Symmetrisch**: Jeder Spieler kann eine Lobby erstellen und als „Ersteller" agieren.  
1. Formular: Spiel, Einsatz, Modus.  
2. Backend: kdapp-Episode + `createMatch` (Einsatz A einbringen).[file:15][file:1]  
3. State: `WAITING_FOR_PLAYER`.

### 3.2 Lobby joinen (zweiter Spieler)

1. Liste zeigt Lobbys in `WAITING_FOR_PLAYER`.  
2. Join → `acceptMatch` + Deposit-Flow für B.[file:1]  
3. State: `PENDING_DEPOSITS`.

### 3.3 Einzahlungen

- Jeder Spieler zahlt einzeln via kaspa-auth.[file:1]  
- State bleibt `PENDING_DEPOSITS` bis beide bestätigt.  
- **Kein Timeout** – für Testmodus.

### 3.4 Zu IN_GAME

- Bei beiden Deposits: `READY_TO_LOCK`.  
- Manueller/automatischer Trigger zu `LOCKED / IN_GAME`.

---

## 4. FaceID (optional)

- Button „FaceID verifizieren" in `READY_TO_LOCK` / `RESOLVING`.  
- Verifiziert Spieler, speichert Hash off-chain.  
- **Kein Blocker**: Match startet auch ohne FaceID.  
- Nutzen: Anti-Fraud-Flag, Dispute-Resolution.[file:1]

---

## 5. Ergebnis & Auszahlung

Oracle fragt FACEIT/etc. ab → `resolveMatch` → Auszahlung.[file:1]

FaceID kann optional vor `RESOLVING` geprüft werden.

---

## 6. UI/API

### 6.1 Frontend

- Lobby-Liste: Nur `WAITING_FOR_PLAYER` joinbar.  
- Detail: Deposits + FaceID-Status (✓/–).  
- Buttons: „Erstellen" (symmetrisch), „Joinen", „Einzahlen", „FaceID" (opt.).

### 6.2 API (Beispiele)

- `POST /lobbies` – Erstellen.  
- `POST /lobbies/{id}/join` – Joinen.  
- `POST /lobbies/{id}/deposit` – Einzahlen.  
- `POST /lobbies/{id}/faceid` – Optional.  
- WebSocket: Status-Updates.

---

## 7. Implementierung

1. kdapp-Episode mit symmetrischem Ersteller/Joiner.[file:15]  
2. Smart-Contract-Calls via rusty-kaspa.[file:1]  
3. No-Timeout-Logik (deaktiviert).  
4. FaceID optional markieren.

