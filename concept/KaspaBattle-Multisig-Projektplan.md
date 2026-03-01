# KaspaBattle – Multisig Escrow Projektablaufplan

**Version 1.0** | **Erstelldatum:** 01.03.2026  
**Option 2:** 2/3 Multisig Native Kaspa Escrow (Trustless P2P Wager System)

---

## 📋 Projekt-Übersicht

### **Ziel**

Implementierung eines **vollständig trustless Escrow-Systems** für KaspaBattle mit:

- ✅ 2/3 Multisig-Adressen (Player1 + Player2 + Platform)
- ✅ Automatisches Payout nach Faceit-Verifizierung
- ✅ Native Kaspa L1 (keine Smart Contracts/L2)
- ✅ Wallet-Integration (KasWare)
- ✅ Explorer-verifizierbar

### **Technologie-Stack**

- **Blockchain:** Kaspa L1 (Native Multisig via Schnorr Signatures)
- **Backend:** Rust (Rusty-Kaspa) / Node.js (kaspa-wasm)
- **Frontend:** React/Vue + KasWare Extension
- **Oracle:** Faceit API Webhook

### **Projektdauer:** 10 Arbeitstage (2 Wochen)

### **Budget:** Open Source (keine externen Kosten)

---

## 🎯 Meilensteine & Timeline

```
Week 1: Core Infrastructure
├─ Day 1-2: Multisig Generator + Backend Setup
├─ Day 3-4: Frontend Deposit Integration
└─ Day 5: End-to-End Tests (Testnet)

Week 2: Oracle + Production
├─ Day 6-7: Faceit Webhook + Payout Logic
├─ Day 8-9: Security Audit + Edge Cases
└─ Day 10: Mainnet Deployment + Monitoring
```

---

## 📦 Phase 1: Backend Infrastructure (Tag 1-2)

### **1.1 Multisig Address Generator**

**Verantwortlich:** Backend Dev  
**Technologie:** Rust (Rusty-Kaspa) oder Node.js (kaspa-wasm)  
**Dauer:** 1 Tag

#### **Aufgaben:**

- [ ] Kaspa RPC Client Setup (Testnet)
- [ ] Multisig-Adresse Generator implementieren
- [ ] API Endpoint: `POST /api/battle/create-escrow`
- [ ] Tests: 3 Pubkeys → 1 Multisig-Adresse

#### **Code-Struktur:**

```rust
// backend/src/escrow/multisig.rs
pub fn create_multisig_address(
    player1_pubkey: PublicKey,
    player2_pubkey: PublicKey,
    platform_pubkey: PublicKey,
    network: NetworkId
) -> Result<Address> {
    let pubkeys = vec![player1_pubkey, player2_pubkey, platform_pubkey];
    let multisig = Multisig::new(pubkeys, 2)?; // 2/3 threshold
    Ok(multisig.to_address(network))
}
```

#### **API Response:**

```json
{
  "battleId": "123",
  "escrowAddress": "kaspa:qr1multisig...",
  "pubkeys": ["kaspa:qp1...", "kaspa:qp2...", "kaspa:qp3..."],
  "threshold": 2
}
```

#### **Tests:**

- ✅ 3 Testnet Pubkeys → gültige Multisig-Adresse
- ✅ Adresse verifizierbar auf Explorer
- ✅ Threshold 2/3 korrekt

---

### **1.2 Partial Transaction Signer**

**Verantwortlich:** Backend Dev  
**Technologie:** Rust (Rusty-Kaspa)  
**Dauer:** 1 Tag

#### **Aufgaben:**

- [ ] UTXO Monitoring (Escrow-Adresse deposits)
- [ ] Payout TX Builder (Winner + Platform Fee)
- [ ] Partial Sign (Platform Key)
- [ ] WebSocket Push zu Winner Frontend

#### **Code-Struktur:**

```rust
// backend/src/escrow/payout.rs
pub async fn generate_payout_tx(
    battle_id: u64,
    winner_address: Address,
    platform_key: PrivateKey
) -> Result<PartialTransaction> {
    let escrow_utxos = rpc.get_utxos_by_address(escrow_address).await?;
    
    let tx = TransactionBuilder::new()
        .add_input(escrow_utxos[0].clone())
        .add_output(winner_address, 190_000_000)  // 1.9 KAS
        .add_output(PLATFORM_ADDRESS, 10_000_000) // 0.1 KAS fee
        .fee_rate(1.0)
        .build()?;
    
    let partial_tx = tx.partial_sign(&platform_key)?;
    Ok(partial_tx)
}
```

#### **WebSocket Payload:**

```json
{
  "type": "PAYOUT_READY",
  "battleId": "123",
  "winner": "kaspa:qp1...",
  "partialTx": "01000000...",
  "amount": 190000000,
  "requiredSigs": 1
}
```

#### **Tests:**

- ✅ 2 KAS Escrow → 1.9 KAS Winner + 0.1 KAS Platform
- ✅ Partial TX hat 1/3 Signatur
- ✅ Serialisierung korrekt (hex format)

---

## 🎨 Phase 2: Frontend Integration (Tag 3-4)

### **2.1 Challenge Create Flow**

**Verantwortlich:** Frontend Dev  
**Technologie:** React/Vue + KasWare  
**Dauer:** 1 Tag

#### **User Flow:**

```
1. Player1: "Challenge erstellen"
   ↓
2. Backend generiert Multisig-Adresse (P1 + ANY + Platform)
   ↓
3. Frontend zeigt QR/Link: "Einzahlen + teile Link"
   ↓
4. Player1: window.kasware.sendKaspa(escrowAddr, 1e8)
   ↓
5. Status: "Warte auf Player2..."
```

#### **Code:**

```typescript
// frontend/src/components/CreateChallenge.tsx
const handleCreateChallenge = async () => {
  const player1Pubkey = await kasware.getPublicKey();
  
  const response = await fetch('/api/battle/create-escrow', {
    method: 'POST',
    body: JSON.stringify({
      player1Pubkey,
      wager: wagerKAS * 1e8,
      faceitMatchId
    })
  });
  
  const { battleId, escrowAddress } = await response.json();
  
  // Deposit
  const txId = await window.kasware.sendKaspa(escrowAddress, wagerKAS * 1e8);
  
  setStatus('deposit_confirmed');
  navigate(`/battle/${battleId}/waiting`);
};
```

#### **UI Components:**

- [ ] Challenge Form (game, wager, players)
- [ ] Escrow Address Display + QR Code
- [ ] Deposit Status (pending → confirmed)
- [ ] Share Link Button

---

### **2.2 Player2 Join Flow**

**Verantwortlich:** Frontend Dev  
**Dauer:** 0.5 Tag

#### **User Flow:**

```
1. Player2 scannt QR / öffnet Link
   ↓
2. Backend updated Multisig (P1 + P2 + Platform)
   ↓
3. Player2: window.kasware.sendKaspa(escrowAddr, 1e8)
   ↓
4. Status: "Battle ready! Beide eingezahlt."
```

#### **Code:**

```typescript
const handleJoinBattle = async (battleId: string) => {
  const player2Pubkey = await kasware.getPublicKey();
  
  await fetch(`/api/battle/${battleId}/join`, {
    method: 'POST',
    body: JSON.stringify({ player2Pubkey })
  });
  
  const { escrowAddress } = await response.json();
  
  const txId = await kasware.sendKaspa(escrowAddress, wagerSompi);
  setStatus('ready');
};
```

---

### **2.3 Winner Payout Flow**

**Verantwortlich:** Frontend Dev  
**Dauer:** 0.5 Tag

#### **User Flow:**

```
1. Faceit Match Ende → WebSocket: "PAYOUT_READY"
   ↓
2. Winner UI: "Du hast gewonnen! Signiere TX."
   ↓
3. window.kasware.completeMultisig(partialTx)
   ↓
4. TX Broadcast → Explorer Link anzeigen
```

#### **Code:**

```typescript
// frontend/src/components/PayoutButton.tsx
const handleSignPayout = async () => {
  try {
    const completeTx = await window.kasware.completeMultisig({
      partialTx: payoutData.partialTx,
      pubkeyIndex: isPlayer1 ? 0 : 1,
      network: 'testnet'
    });
    
    const txId = await rpc.submitTransaction(completeTx);
    
    toast.success(`Gewinn ausgezahlt! TX: ${txId}`);
    navigate(`/battle/${battleId}/won`);
    
  } catch (err) {
    toast.error('Signieren fehlgeschlagen: ' + err.message);
  }
};
```

#### **UI Components:**

- [ ] Payout Notification Badge
- [ ] TX Preview (Input/Output)
- [ ] "Signieren" Button (KasWare)
- [ ] Success Screen + Explorer Link

---

## 🧪 Phase 3: Testing & QA (Tag 5)

### **3.1 End-to-End Tests (Testnet)**

**Verantwortlich:** QA + DevOps  
**Dauer:** 1 Tag

#### **Test-Szenarien:**

- [ ] **Happy Path:** P1 create → P2 join → beide deposit → Faceit → Winner claim
- [ ] **Deposit Timeout:** P2 joined nicht → Refund nach 24h
- [ ] **Wrong Amount:** P2 sendet 0.5 KAS statt 1 KAS → rejected
- [ ] **Network Switch:** Wallet auf Mainnet, App auf Testnet → Fehler
- [ ] **Double Claim:** Winner versucht 2x claim → denied

#### **Tools:**

- Kaspa Testnet Faucet
- Explorer Monitoring
- Playwright (E2E Browser Tests)

---

## 🔗 Phase 4: Faceit Oracle Integration (Tag 6-7)

### **4.1 Faceit Webhook Handler**

**Verantwortlich:** Backend Dev  
**Dauer:** 1 Tag

#### **Aufgaben:**

- [ ] Webhook Endpoint: `POST /webhook/faceit`
- [ ] Match Result Parsing (winner detection)
- [ ] Trigger Partial TX Generation
- [ ] WebSocket Push zu Winner

#### **Code:**

```rust
// backend/src/webhook/faceit.rs
#[post("/webhook/faceit")]
async fn faceit_webhook(payload: Json<FaceitMatchResult>) -> impl Responder {
    let battle = db.get_battle_by_faceit_id(payload.match_id)?;
    
    let winner = if payload.teams[0].score > payload.teams[1].score {
        battle.player1_address
    } else {
        battle.player2_address
    };
    
    let partial_tx = generate_payout_tx(battle.id, winner, PLATFORM_KEY).await?;
    
    websocket.send_to_user(winner, PayoutReady { partial_tx });
    
    HttpResponse::Ok().finish()
}
```

#### **Sicherheit:**

- [ ] Webhook Signature Verifizierung (Faceit HMAC)
- [ ] Rate Limiting (1 webhook/match)
- [ ] Idempotency (duplicate webhook = noop)

---

### **4.2 Manual Fallback**

**Verantwortlich:** Backend Dev  
**Dauer:** 0.5 Tag

#### **Admin Dashboard:**

```typescript
// Admin Interface für manuelle Resolution
POST /admin/battle/{id}/resolve
Body: { winner: "kaspa:qp1...", faceitMatchId: "12345" }

→ Generiert Partial TX wie Webhook
```

**Use Cases:**

- Faceit API down
- Dispute Resolution (24h timelock)
- Emergency Payout

---

## 🔒 Phase 5: Security Audit (Tag 8-9)

### **5.1 Smart Contract-ähnliche Prüfung**

**Verantwortlich:** Security Lead  
**Dauer:** 1 Tag

#### **Checklist:**

- [ ] **Reentrancy:** Partial TX kann nicht 2x signiert werden
- [ ] **Amount Validation:** Exact wager required (kein mehr/weniger)
- [ ] **Key Management:** Platform Key in HSM/Vault (nicht in Code)
- [ ] **Timelock:** Refund nach 24h wenn kein P2 deposit
- [ ] **Multisig Threshold:** 2/3 korrekt (nicht 1/3 oder 3/3)
- [ ] **Explorer Verifizierung:** TX outputs stimmen mit Plan

---

### **5.2 Edge Case Handling**

**Verantwortlich:** DevOps + Backend  
**Dauer:** 1 Tag

#### **Szenarien:**

1. **P2 joined nicht (24h):**

   ```
   Refund P1: Multisig TX (Platform + P1) → P1 Address
   ```

2. **Faceit API Error:**

   ```
   Manual Admin Resolution (Dashboard)
   ```

3. **Winner offline:**

   ```
   Partial TX bleibt 7 Tage gültig
   Email/Push Notification
   ```

4. **Dispute (beide claimen Winner):**

   ```
   Faceit API re-check
   24h Timelock + Admin Override
   ```

5. **Wallet Disconnect mid-sign:**

   ```
   Partial TX re-send via WebSocket
   ```

---

## 🚀 Phase 6: Production Deployment (Tag 10)

### **6.1 Mainnet Launch**

**Verantwortlich:** DevOps  
**Dauer:** 0.5 Tag

#### **Deployment Steps:**

1. **Backend:**

   ```bash
   docker build -t kaspabattle-backend:v1.0 .
   docker push registry/kaspabattle-backend:v1.0
   kubectl apply -f k8s/production.yaml
   ```

2. **Frontend:**

   ```bash
   npm run build -- --mode production
   vercel deploy --prod
   ```

3. **Environment Variables:**

   ```env
   KASPA_NETWORK=mainnet
   KASPA_RPC=wss://mainnet-rpc.kaspa.org:16110
   PLATFORM_KEY_VAULT=vault://platform-multisig-key
   FACEIT_WEBHOOK_SECRET=xxx
   ```

---

### **6.2 Monitoring Setup**

**Verantwortlich:** DevOps  
**Dauer:** 0.5 Tag

#### **Metrics:**

- [ ] Escrow Deposits per Hour (Grafana)
- [ ] Payout Success Rate (%)
- [ ] Faceit Webhook Latency (ms)
- [ ] KasWare Signature Success Rate
- [ ] Explorer TX Confirmation Time

#### **Alerts:**

- 🚨 Escrow balance mismatch
- 🚨 Partial TX generation failure
- 🚨 Faceit webhook timeout (>30s)
- 🚨 Platform key HSM unreachable

#### **Tools:**

- Prometheus + Grafana
- Sentry (Error Tracking)
- LogRocket (Frontend Session Replay)

---

## 📊 Ressourcen-Planung

### **Team:**

- **Backend Dev (Rust):** 1 Person, 5 Tage
- **Frontend Dev (React/Vue):** 1 Person, 3 Tage
- **DevOps:** 1 Person, 2 Tage
- **QA/Security:** 1 Person, 2 Tage

### **Infrastruktur:**

- **Kaspa Testnet RPC:** Free (public nodes)
- **Kaspa Mainnet RPC:** Free (public nodes) oder Self-hosted ($50/mo VPS)
- **Backend Hosting:** AWS/GCP ($30/mo)
- **Frontend Hosting:** Vercel Free Tier
- **Monitoring:** Grafana Cloud Free Tier

### **Total Budget:** $80/Monat (Production)

---

## 📈 Success Metrics (KPIs)

### **Phase 1 (Week 1):**

- ✅ 10+ Testnet Battles erfolgreich (Deposit → Payout)
- ✅ 0 failed partial TX signatures
- ✅ Average payout time < 30 seconds

### **Phase 2 (Week 2):**

- ✅ 100+ Mainnet Battles in 1. Woche
- ✅ 99.5% Payout Success Rate
- ✅ 0 disputed payouts
- ✅ Explorer-verified: 100% correct outputs

---

## 🛡️ Risk Management

| Risk | Wahrscheinlichkeit | Impact | Mitigation |
|------|-------------------|--------|------------|
| **Faceit API down** | Mittel | Hoch | Manual Admin Fallback |
| **KasWare Multisig Bug** | Niedrig | Hoch | E2E Tests, Fallback WASM SDK |
| **Platform Key Leak** | Niedrig | Kritisch | HSM/Vault, Key Rotation |
| **UTXO Race Condition** | Niedrig | Mittel | Atomic TX locking |
| **Network Congestion** | Niedrig | Niedrig | Dynamic Fee Estimation |

---

## 📚 Dokumentation

### **User Documentation:**

- [ ] **Battle Creation Guide** (mit Screenshots)
- [ ] **Payout Signing Tutorial** (KasWare)
- [ ] **Troubleshooting** (häufige Fehler)
- [ ] **FAQ** (Multisig Sicherheit)

### **Developer Documentation:**

- [ ] **API Reference** (Swagger/OpenAPI)
- [ ] **Multisig Architecture** (Diagramme)
- [ ] **Deployment Guide** (Mainnet Setup)
- [ ] **Security Best Practices**

---

## ✅ Go-Live Checklist

### **Pre-Launch (Day 9):**

- [ ] Security Audit abgeschlossen
- [ ] All E2E Tests passed (>95% coverage)
- [ ] Mainnet Platform Key in HSM
- [ ] Faceit Webhook live + tested
- [ ] Monitoring Dashboards live
- [ ] Rollback Plan dokumentiert

### **Launch (Day 10):**

- [ ] Backend deployed (Production)
- [ ] Frontend deployed (Production)
- [ ] DNS updated (kaspabattle.com)
- [ ] Status Page live (status.kaspabattle.com)
- [ ] Team on-call (24h)

### **Post-Launch (Day 11-14):**

- [ ] 24/7 Monitoring (1 Woche)
- [ ] Daily KPI Review
- [ ] User Feedback sammeln
- [ ] Bug Hotfixes (Priority: Critical → High → Medium)

---

## 📞 Kontakt & Support

**Projekt Lead:** [Name]  
**Backend:** [Name] (Rust/Kaspa)  
**Frontend:** [Name] (React/KasWare)  
**DevOps:** [Name] (AWS/Docker)  

**Status Updates:** Daily Standups (9:00 CET)  
**Issue Tracking:** GitHub Projects  
**Chat:** Discord #kaspabattle-dev

---

## 🎉 Success Criteria

**MVP Erfolg (Tag 10):**

- ✅ 1 erfolgreicher Mainnet Battle (End-to-End)
- ✅ Explorer zeigt korrekte Multisig-TX
- ✅ Gewinner erhält 1.9 KAS innerhalb 60s

**Production Erfolg (1 Monat):**

- ✅ 1000+ abgeschlossene Battles
- ✅ 99.9% Uptime
- ✅ 0 ungerechtfertigte Disputes
- ✅ Community Feedback >4.5/5⭐

---

**Dokument-Version:** 1.0  
**Letzte Aktualisierung:** 01.03.2026  
**Nächstes Review:** 08.03.2026 (Nach Week 1)
