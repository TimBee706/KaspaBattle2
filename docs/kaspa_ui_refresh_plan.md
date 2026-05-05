# KaspaBattle – Globales UI-Redesign (Gamer / Glassmorphism)

## 0. Zielbild

Das Ziel ist ein einheitliches, modernes UI für die gesamte KaspaBattle-Webapp, das:

- klar als Gaming-/Esports-Produkt erkennbar ist (dunkles Theme, Akzentfarben, starke Typografie),
- einen dezenten, hochwertigen Glassmorphism-Look verwendet (gläserne Karten/Buttons, aber nicht überladen),
- konsequent ohne Emojis auskommt und stattdessen hochwertige, einfarbige SVG-Icons nutzt,
- eine durchgängige Typografie-Hierarchie definiert (Headlines, Subheads, Body, Labels),
- alle Seiten (Landing, Lobby, Match, Wallet, Profile, Support, Turniere, History usw.) optisch zusammenführt.

Dieses Dokument ist für einen Coding Agent gedacht, der auf Basis des Branches `TournamentUpdate` einen neuen Feature-Branch anlegt und die Design-Anpassungen implementiert.

---

## 1. Designsystem – Farben, Flächen, Typografie

### 1.1 Farbpalette

**Primäre Farben** (bereits vorhanden, werden geschärft):

- `--color-bg`: sehr dunkles Blau/Anthrazit (z.B. `#050812`), als Page-Background.
- `--color-surface`: dunkle, leicht bläuliche Paneele (z.B. `rgba(13, 27, 42, 0.9)`), für Karten.
- `--color-accent-primary`: Kaspa-Grün/Türkis (bestehendes `#49EACB`).
- `--color-accent-secondary`: leicht violette Akzentfarbe für sekundäre Highlights (z.B. `#7C3AED`).

**Statusfarben** (für Badges, Hinweise):

- Success: `#22C55E` (Grün)
- Warning: `#F59E0B` (Orange)
- Danger: `#EF4444` (Rot)
- Info: `#3B82F6` (Blau)

**Textfarben:**

- Primär: `#F9FAFB` (fast weiß)
- Sekundär: `#9CA3AF` (grau, Sekundärinfos)
- Muted: `#6B7280` (Labels, Meta)

**Umsetzung:**

- Alle harten Hex-Werte in den TSX-Dateien schrittweise auf CSS-Variablen oder Tailwind-Konfiguration angleichen (soweit sinnvoll), um Wiederverwendung sicherzustellen.

### 1.2 Glassmorphism-Flächen

Neue generische Utility-Styles (als Tailwind-Klassen-Kombinationen oder globale CSS-Klassen):

- **`glass-panel`** (für Cards / Container):
  - Hintergrund: `bg-white/5` oder `bg-kaspa-card/40` + `backdrop-blur-md`.
  - Border: `border border-white/10` oder `border border-kaspa-border`.
  - Shadow: `shadow-[0_18px_60px_rgba(0,0,0,0.65)]`.
  - Radius: `rounded-2xl`.

- **`glass-button`** (Sekundär-Button):
  - Hintergrund: `bg-white/5`.
  - Border: `border border-white/15`.
  - Hover: `hover:bg-white/10 hover:border-white/30`.
  - Text: `text-gray-100`.

Alle bestehenden Karten/Boxen Schritt für Schritt auf diese Patterns umstellen, insbesondere:

- LandingPage-Abschnitte (Hero, What is, Why Kaspa, Future),
- Lobby Container (`LobbyPage`),
- Match-/Escrow-Karten,
- Wallet-/Profile-Karten,
- Turnier-Karten und -Detailboxen.

### 1.3 Typografie-System

Festlegung einer konsistenten Typo-Hierarchie:

- **H1 (Hero / wichtigste Headline)**
  - `text-5xl md:text-6xl`, `font-black`, `tracking-tight`, uppercase optional.
- **H2 (Sektionstitel)**
  - `text-3xl md:text-4xl`, `font-black`.
- **H3 (Box-/Card-Titel)**
  - `text-xl md:text-2xl`, `font-bold`.
- **H4/Label**
  - `text-sm`, `font-semibold`, `uppercase`, `tracking-widest`, `text-gray-400`.
- **Body**
  - `text-sm md:text-base`, `text-gray-300`, Zeilenhöhe `leading-relaxed`.

Umsetzungsschritte:

1. Hero-Headline auf LandingPage beibehalten, aber Subheadline (siehe Copy unten) modernisieren.
2. WHY KASPA / WAS IST KASPABATTLE / FUTURE-Section auf H2/H3-System umstellen, ohne wechselnde, inkonsistente Größen.
3. Alle Überschriften in Lobby/Match/Turnier-Seiten auf dieses Raster mappen (z.B. Page-Titel = H1/H2, Card-Titel = H3).

---

## 2. Icon- & Emoji-Strategie

### 2.1 Entfernen von Emojis

Anforderung: Alle Emojis verschwinden, u.a.:

- LandingPage: ⚡, 🏎️, 💸, 🛡️ in WHY KASPA Cards, 🏆 in Tournament-Callout.
- Turnier-Seiten: Emojis in Filter-Labels, Buttons (Ergebnisse, Disput, etc.), Badges (Sieger-Icon, Disput), Admin-Ansicht.
- Sonstige Seiten: Profil-Hinweise, Match-Status, Testmodus-Hinweise usw.

### 2.2 SVG-Icons statt Emojis

Plan:

1. **Icons-Ordner**: `/public/icons` oder `/src/assets/icons` einführen.
2. Pro Themenbereich minimalistische, einfarbige Icons im Outline- oder Duotone-Stil nutzen (z.B. aus einem Premium-Set oder eigenen SVGs):
   - Geschwindigkeit / Finalität → stilisierter Blitz.
   - Blöcke / Skalierung → Block-DAG-Icon / gestapelte Blöcke.
   - Gebühren → stilisierte Münzen / KAS-Logo abstrahiert.
   - Sicherheit → Schild.
   - Turnier / Sieger → stilisierte Trophäe ohne Text.

3. **`Icon`-Komponente** (optional):
   - `<Icon name="shield" className="w-6 h-6 text-kaspa-primary" />`, die interne `useMemo`/Map auf verschiedene SVGs macht.

4. Alle bisherigen Emojis durch SVG-Icons ersetzen:
   - Statt `⚡` in WHY KASPA Card: `<Icon name="bolt" />`.
   - Statt `🏎️`: `<Icon name="speed" />`.
   - Statt `💸`: `<Icon name="fees" />`.
   - Statt `🛡️`: `<Icon name="shield" />`.

---

## 3. Content & Copy-Updates (insb. LandingPage)

### 3.1 Hero-Section

Aktuelles Beispiel (Englisch):

> THE FUTURE OF ESPORTS  
> COMPETE ON THE FASTEST BLOCKCHAIN IN THE WORLD

Das wirkt zu hoch gegriffen. Vorschlag für eine bodenständigere, aber trotzdem starke Message:

**Englisch:**

- H1 (bleibt in der Art):
  - "PLAY TO WIN KASPA" (wie aktuell, aber mit neuem Subtext)
- Subheadline neu:
  - Alt: "The world’s first decentralized gaming wager platform on Kaspa…"  
  - Neu: "Competitive 1v1 matches and tournaments with non-custodial KAS escrows and instant, trustless payouts. Built for real players, not casinos."

**Deutsch:**

- H1: "PLAY TO WIN KASPA" (Englisch ist okay als Brand-Headline).
- Subheadline neu:
  - Alt: sehr marketinglastig.  
  - Neu: "Spiele kompetitive 1vs1-Matches und Turniere mit KAS-Einsatz – voll dezentral, non‑custodial und automatisch ausgezahlt. Für Gamer, nicht für Casinos."

Umsetzung: in `LandingPage.tsx` die entsprechenden i18n-Keys (`hero.subtitle`) aktualisieren.

### 3.2 "Was ist KaspaBattle" / "What is KaspaBattle"-Section

Ziel: den Text etwas klarer, kürzer und gamer-orientiert formulieren.

- Kürzerer Absatz, der in 2–3 Sätzen erklärt:
  - P2P-Wetten zwischen Spielern,
  - Escrow auf Kaspa,
  - FACEIT-Integration.

Beispiel (DE):

> "KaspaBattle ist eine dezentrale Plattform für 1vs1 Esports-Wetten auf Kaspa. Zwei Spieler zahlen ihren Einsatz in einen sicheren Escrow ein, spielen ihr Match (z.B. CS2 auf FACEIT) und unser Oracle zahlt automatisch den Gewinner aus. Ohne Zwischenhändler, ohne Custody, 100% on-chain."

### 3.3 WHY KASPA – Rework ohne Emojis

Aktuell:

- Titel "WHY KASPA?" und vier Cards mit Emojis.

Ziel: Hochwertige Section mit minimalistischen Icons und neutralem Copy.

**Neues Layout:**

- Titel: "WHY KASPA?" (H2).
- Grid mit 2x2 Cards, jede Card:
  - kleiner SVG-Icon oben (Kaspa-Primärfarbe),
  - H3-Titel,
  - kurzer Text (1–2 Sätze).

**Neue Texte (EN, sinngemäß):**

1. Finality:
   - Title: "Fast finality"
   - Text: "Kaspa confirms transactions in around one second – ideal for real-time esports payouts."

2. Throughput:
   - Title: "High throughput"
   - Text: "Up to 10 blocks per second with BlockDAG – built to handle many concurrent matches."

3. Low fees:
   - Title: "Low fees"
   - Text: "Tiny transaction costs make even small wagers practical for players."

4. Security:
   - Title: "Proof-of-Work security"
   - Text: "Kaspa’s PoW consensus provides strong security and decentralization for your funds."

Die deutschen Texte entsprechend nüchtern übersetzen und in `common.json` aktualisieren.

### 3.4 Future-Section ("DIE ZUKUNFT DES ESPORTS")

Aktuell (DE, sinngemäß):

> DIE ZUKUNFT DES ESPORTS  
> MISSE DICH AN DER SCHNELLSTEN BLOCKCHAIN DER WELT

Ziel: weniger Buzzword, mehr Fokus auf Produkt.

**DE Vorschlag:**

- H2: "DIE ZUKUNFT VON COMPETITIVE KASPA GAMING" oder "DEZENTRALE ESPORTS-WETTEN AUF KASPA".
- Subheadline: "Spiele um KAS, ohne Vermittler und ohne Custody. Transparent, schnell und fair."

**EN Vorschlag:**

- H2: "Competitive gaming on Kaspa"
- Subheadline: "Play for KAS with non-custodial escrows and instant, trustless payouts."

Buttons in dieser Section (Lobby, Tournaments, Wallet/Faceit) optisch an neue Button-Styles anpassen (siehe unten).

---

## 4. Komponenten-Styles (Buttons, Badges, Karten)

### 4.1 Buttons

Definiere im Code (oder in einer zentralen Komponente) 3–4 Button-Varianten:

1. **Primary** (wichtigste Call-to-Action)
   - Background: `bg-[#49EACB]`.
   - Text: `text-[#050812]`.
   - Radius: `rounded-xl`.
   - Shadow: starker Glüheffekt (`shadow-[0_0_32px_rgba(73,234,203,0.45)]`).
   - Hover: etwas heller + stärkere Shadow.

2. **Secondary (glass)**
   - Background: `bg-white/5`.
   - Border: `border border-white/15`.
   - Text: `text-gray-100`.
   - Hover: `hover:bg-white/10 hover:border-white/30`.

3. **Tertiary / Ghost**
   - Kein Hintergrund, nur Text + Underline on hover oder dezente Border.

4. **Danger**
   - Background: `bg-red-600/20`.
   - Border: `border border-red-500/40`.
   - Text: `text-red-300`.

Umsetzung:

- Optional: `<Button variant="primary" />` Komponente einführen und überall verwenden.
- Ansonsten Tailwind-Utility-Kombinationen als Konvention dokumentieren und alle bestehenden Buttons in Landing/Lobby/Match/Tournaments/Walle/Profile entsprechend refactoren.

### 4.2 Status-Badges

- Einheitliche Badge-Komponente (z.B. `StatusBadge`) für Match-Status, Lobby-Status, Turnier-Status.
- Design:
  - `inline-flex items-center gap-2 px-3 py-1 rounded-full border text-xs font-semibold`.
  - Hintergrund: `bg-white/5` + Border in Statusfarbe.
  - Kleiner runder Statuspunkt (kein Emoji): `<span className="w-1.5 h-1.5 rounded-full bg-current" />`.

### 4.3 Karten & Panels

- Alle Informations-Boxen (Match-Info, Escrow, Turnier-Infos, Wallet-Karten, Profile-Statistik) auf `glass-panel`-Pattern umstellen (siehe 1.2).
- Gleiche Abstände: `p-5` oder `p-6`, `space-y-2`/`space-y-3`.

---

## 5. Seite für Seite – Implementierungsschritte

### 5.1 LandingPage.tsx

1. **Hero:**
   - Subheadline-Text über i18n austauschen (siehe 3.1).
   - ggf. kleinere Anpassung an H1-Größe (nicht größer als nötig, Fokus auf Lesbarkeit).

2. **"Was ist KaspaBattle"-Section:**
   - Container mit `glass-panel`-Styles.
   - Text wie in 3.2 vorgeschlagen.

3. **How 1vs1 Works:**
   - Step-Cards visuell an Glas-Pattern anpassen (weniger harte Ränder, mehr Blur, einheitliche Icons oder Nummernkreise).

4. **WHY KASPA:**
   - Cards auf Icon + H3 + Text umstellen.
   - Emojis entfernen.
   - Texte an neue Copy anpassen.

5. **Future-Section:**
   - Headline/Subheadline anpassen (3.4).
   - Buttons auf neue Button-Varianten umstellen.

6. **Tournament Mode Callout:**
   - Design an Glas-Stil anpassen.
   - Emojis entfernen, SVG-Trophy nutzen.
   - Text: neutraler, weniger Marketing (z.B. "Run community tournaments with automated prize pools.").
   - Hinweis: Sichtbarkeit weiterhin an `testMode`-Flag binden (siehe bestehende Logik aus vorherigem Plan).

### 5.2 LobbyPage.tsx

- Page-Container: `min-h-screen bg`, `max-w-5xl mx-auto`, etc. mit restlicher App alignen.
- Header: Title/Subtitle-Typo an Landing/Lobby-Pattern angleichen.
- Action-Button "Challenge erstellen" auf Primary-Button-Style setzen.
- Testmode-Badge: gläsernes Panel mit Icon anstatt Emoji.

### 5.3 MatchPage, EscrowPage

- Alle Statuskarten (Deposit, Status, Match Info) auf `glass-panel`-Design.
- Match-Status-Anzeigen auf konsistente Status-Badges umstellen (ohne Emojis).

### 5.4 WalletPage & ProfilePage

- Profile-Statistik-Kacheln (Wins, Losses, Winrate, etc.) in 2x2 oder 3x2 Glas-Cards.
- Wallet-Bereich mit gläsernem Panel für Balance, Adresse, Backup.
- Emojis in Warntexten entfernen und durch Icons bzw. reine Typografie ersetzen.

### 5.5 Turnier-Seiten (List, Detail, Admin, Results)

- Auf dem bereits existierenden Turnier-Layout aufbauen (aus dem vorherigen Tournament-Update-Plan) und:
  - alle Emojis entfernen,
  - Buttons und Karten auf neue Button-/Panel-Styles umstellen,
  - Status-Badges vereinheitlichen,
  - Copy (z.B. Disput-Texte, Admin-Hinweise) leicht straffen.

### 5.6 History-/Support-/Terms-/Whitepaper-Seiten

- Überschriften auf H1/H2-Schema bringen.
- Content in gläserne Panels packen (insbesondere rechtliche Texte), leicht lesbar (Breite begrenzen, größere Zeilenhöhe).

---

## 6. Technische Umsetzung & Reihenfolge für den Coding Agent

1. **Branch anlegen**
   - Basierend auf `TournamentUpdate` einen neuen Feature-Branch erstellen, z.B.:  
     - `git checkout TournamentUpdate`  
     - `git checkout -b feature/ui-refresh`

2. **Design Tokens & Utilities**
   - Globale CSS / Tailwind-Konfiguration um die beschriebenen Farben, Radii und Shadows ergänzen.
   - Utility-Klassen oder Komponenten (`glass-panel`, `glass-button`, `Button`, `StatusBadge`) einführen.

3. **LandingPage zuerst refactoren**
   - Hier alle neuen Patterns ausprobieren und stabilisieren (Hero, Why Kaspa, Future, Tournament-Callout).

4. **Navigation/Header/Footer anpassen**
   - Einheitlicher Header (Logo, Navigation, Wallet/Profil), gläserner Hintergrund.
   - Footer auf neues Design bringen (dezenter, klare Links, kein Emoji).

5. **Kernseiten sequenziell migrieren**
   - Reihenfolge z.B.: Lobby → Match → Escrow → Wallet → Profile → Tournaments → History → Support/Terms.
   - Nach jeder Seite visuell gegen das neue Designsystem prüfen.

6. **Emoji-Cleanup & Icon-Integration**
   - Suche über das Projekt (`grep`/IDE) nach Emojis und alle ersetzen.
   - SVG-Icon-Set einbinden, Mapping in einer zentralen Komponente pflegen.

7. **Copy-Updates über i18n**
   - Alle oben erwähnten Texte in `battle-frontend/src/locales/*/common.json` aktualisieren (DE & EN).
   - Keine Keys umbenennen, nur Werte.

8. **QA & Responsive Check**
   - Auf Desktop, Tablet, Mobile alle Hauptpfade (Landing, Lobby, Match, Tournament) prüfen.
   - Fokus-States und Kontrastverhältnisse checken (Accessibility).

---

## 7. Abnahmekriterien

- Keine Emojis mehr im UI, alle Symbole sind SVG-Icons im einheitlichen Stil.
- Alle Seiten verwenden konsistente Farb- und Typografie-Patterns.
- LandingPage vermittelt das Produkt klar und weniger übertrieben, insbesondere in Hero-, Why-Kaspa- und Future-Sections.
- Buttons, Karten und Badges sehen überall gleichwertig aus (kein Mischmasch aus alten und neuen Stilen).
- Gamer-Zielgruppe erkennt das UI als modernes Esports-/Gaming-Produkt (dunkles Theme, klare Akzente, hochwertige Optik).

