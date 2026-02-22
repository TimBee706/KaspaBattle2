# Security Design & Roadmap

This document outlines the security architecture of the KaspaBattle MVP and the roadmap for transitioning to the fully trustless vision defined in the Whitepaper.

## Status Overview

The current implementation (v0.3.0) has been hardened against immediate vulnerabilities identified in the 2026 Audit. While some architectural components are currently centralized for MVP speed, clear paths to decentralization are established.

## 1. Escrow Architecture (F-001)

### Current Implementation: Off-Chain Custody

- **Mechanism**: Every match has a unique Kaspa address derived from a master mnemonic using BIP44.
- **Custody**: The server holds the derivation master key in memory (zeroized on drop). The server signs payouts once the Oracle resolves a match.
- **Risk**: A total server compromise allows theft of current escrowed funds.

### Roadmap to Trustless Escrow

1. **Phase 2**: Implement 2-of-2 multi-sig UTXO scripts. Funds require both the Server signature AND the User signature to move.
2. **Phase 3**: Migrate to **Kasplex L2** smart contracts for non-custodial holding of KAS, triggered by Oracle proofs.

---

## 2. Oracle System (F-002)

### Current Implementation: Single-Source Oracle

- **Mechanism**: A single service fetches results from the FACEIT API.
- **Auth**: Oracle endpoints require an API key verified in constant-time (F-007).
- **Proof**: Results are signed with an Ed25519 key for on-chain verifiability.

### Roadmap to Decentralized Oracle

1. **Phase 2**: Multi-Node Consensus. Deploy ≥ 3 nodes. Payout is only triggered if 3/5 nodes report the same result.
2. **Phase 3**: Economic Security. Implement staking/slashing. Oracle nodes must stake KAS to participate and lose stake if they report false data.

---

## 3. User Authentication Security (F-020)

### Current Implementation: Secure Accounts

- **Password Hashing**: Uses **Argon2id** (the winner of the Password Hashing Competition) with unique salts for every user.
- **Session Security**: High-entropy session tokens stored securely in a relational database.
- **OAuth Safety**: Implements **PKCE (Proof Key for Code Exchange)** for the FACEIT OAuth flow to prevent authorization code interception attacks.
- **CSRF Protection**: State-based verification for all cross-origin authentication flows.

---

## 4. Implemented Security Controls

| ID | Control | Purpose |
| :--- | :--- | :--- |
| **F-003** | Mnemonic Zeroization | Master keys are wiped from memory after initialization. |
| **F-005** | CORS Hardening | Restricts API access to authorized frontend origins. |
| **F-006** | Rate Limiting | Prevents DoS and brute-force attacks on the API. |
| **F-007** | Constant-Time Auth | Prevents timing side-channel attacks on Oracle keys. |
| **F-015** | Environment Enforcement | System refuses to start without critical security config (Treasury/Keys). |
| **F-018** | State Machine Guard | Payouts represent an atomic end-state; blocked in any other state. |
| **F-020** | Argon2id Hashing | Protects user passwords against GPU-based brute-force attacks. |
| **F-021** | OAuth PKCE | Secures the FACEIT linking flow against intercept attacks. |

---
[Kaspa Integration ←](kaspa-integration.md) | [Home ↑](../README.md)
