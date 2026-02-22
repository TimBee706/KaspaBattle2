# Security

Security is the highest priority for KaspaBattle. As a platform handling user funds (KAS), we implement rigorous protections at every layer.

## Non-Custodial Principles

KaspaBattle follows the principle of **least trust**:

- **Escrow Separation**: Funds for Match A are never co-mingled with Match B.
- **Short-Lived Custody**: Escrow private keys are used only at the moment of payout/refund and are then purged from memory.
- **Player-Controlled Wallets**: Users sign their own deposit transactions in their own browser wallet.

## Audit Fixes (v0.3.0)

A recent security audit identified several critical areas that have been fully addressed:

- **F-001: Payout Tx Stub**: Replaced mock transaction IDs with real, Schnorr-signed Kaspa transactions.
- **F-002: Resolve Match Auth**: Implemented API Key authentication for the Oracle endpoint.
- **F-006: Timeout Refund**: Added automated refunds for deposits on expired/unjoined matches.
- **F-008: Deposit Attribution**: Fixed a bug where one player could "double-fund" a match using two UTXOs. Now correctly verifies per-address contributions.
- **F-009: Dispute State**: Introduced a `Disputed` state that immediately freezes payouts upon a player's report.

## Oracle Security

The system relies on external game data. We secure this via:

- **Authenticated Push**: Only authorized Oracle keys can call the resolve endpoint.
- **Double Confirmation**: The Oracle fetches results twice from independent platform endpoints (where available) before signing.
- **Roadmap**: Decentralized Oracle Federation where multiple signatures are required for a payout.

## Wallet Security

- **No Server-Side Mnemonics**: The backend never sees your personal wallet mnemonic or private key.
- **Transport Security**: All API communication is encrypted via TLS.
- **Secure Key Management**: Backend escrow keys are derived deterministically and handled using secure memory patterns.

---
[Kaspa Integration ←](kaspa-integration.md) | [Contributing →](contributing.md)
