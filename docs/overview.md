# Project Overview

KaspaBattle is designed to revolutionize competitive gaming by bringing transparency and decentralization to esports wagering.

## The Problem

Esports wagering today suffers from two major issues:

1. **Centralization/Trust**: Players must trust a third-party platform to hold their funds and accurately report match results.
2. **Speed**: Withdrawals and payouts can take days due to legacy banking and manual verification processes.

## The Solution

KaspaBattle solves these by:

1. **Non-Custodial Escrow**: Using per-match unique addresses on the Kaspa DAG. Users interact with the protocol without handing over custody of their KAS until the match is resolved.
2. **Oracle Automation**: Authenticated Oracles pull data directly from gaming platforms (like FACEIT) to resolve matches instantly.
3. **Kaspa DAG**: Leveraging the fastest PoW blockchain to provide sub-second block times and near-instant transaction finality.

## The Match Cycle

A typical wager follows these 8 steps:

1. **Create Challenge**: Player A creates a match with specific wager amounts and game rules.
2. **Join Match**: Player B joins the challenge.
3. **Escrow Generation**: The system generates a unique Kaspa escrow address for the match.
4. **Deposits**: Both players send their KAS wager to the escrow address.
5. **Locking**: The Backend Watcher detects both deposits and locks the match.
6. **Match Play**: The players compete in the linked game (e.g., a CS2 match on FACEIT).
7. **Resolution**: The Oracle fetches the match result and signs a resolution transaction.
8. **Payout**: The platform automatically splits the escrow: 95% to the winner, 5% to the platform (treasury).

## Target Audience

- **Competitive Amateurs**: Players who want "skin in the game" for their daily matches.
- **Pro Teams**: High-stakes competitive matches with guaranteed payouts.
- **Streamers**: Interactive wagering for their communities.

## Technical Foundations

KaspaBattle is built for performance:

- **Scalability**: Kaspa's 10 BPS (and future higher rates) ensures we can handle thousands of concurrent matches.
- **Safety**: Rust provides memory safety and high performance for the core logic.
- **Accessibility**: A browser-based React frontend allows anyone with a KAS wallet to participate.

---
[← Back to README](../README.md) | [Architecture →](architecture.md)
