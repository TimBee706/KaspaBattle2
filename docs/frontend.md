# Frontend Documentation

The KaspaBattle frontend is a React application that provides a responsive interface for players to manage challenges, connect their wallets, and monitor match status.

## Tech Stack

- **Framework**: React 18
- **Build Tool**: Vite
- **State Management**: Zustand
- **Routing**: React Router 6
- **Styling**: Tailwind CSS
- **Kaspa Integration**: `kaspa-wasm` SDK

## Key Components

### 1. Wallet Provider

Integrates the Kaspa WASM SDK.

- **Connection**: Detects and connects to browser wallets or manages local BIP-39 mnemonics.
- **Synchronization**: Uses wRPC to listen for incoming transactions and balance updates.

### 2. Match Lobby

A real-time list of available challenges.

- **Filtering**: Allows users to find matches by stake or game type.
- **Real-time Updates**: Polling or wRPC hooks to show status changes instantly.

### 3. Escrow Interface

Displays the unique deposit address for a match.

- **QR Codes**: Easy deposit via mobile wallets.
- **Status Progress**: Visual indicator for "A deposited", "B deposited", and "Locked".

## Feature Highlights

- **Kaspa WASM Integration**: We use the official Kaspa WASM bindings for sub-second synchronization and transaction signing.
- **FACEIT Integration**: Users log in via FACEIT OAuth to link their gaming profiles.
- **Responsive Design**: Optimized for both desktop (during play) and mobile (to monitor status/deposits).

## Project Structure

```text
src/
├── components/   # Reusable UI elements (Buttons, Cards, Modals)
├── hooks/        # Custom React hooks (useKaspa, useMatch)
├── pages/        # Main route views (Lobby, MatchDetail, Profile)
├── store/        # Zustand state stores
├── types/        # TypeScript interfaces
└── utils/        # Kaspa helpers, formatting, etc.
```

## Running Locally

1. Navigate to the frontend folder: `cd battle-frontend`.
2. Install dependencies: `npm install`.
3. Start the dev server: `npm run dev`.
4. Build for production: `npm run build`.

---
[Backend ←](backend.md) | [Kaspa Integration →](kaspa-integration.md)
