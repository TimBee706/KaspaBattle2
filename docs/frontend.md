# Frontend Documentation

The KaspaBattle frontend is a React/TypeScript SPA for user interaction.

## Tech Stack

- **Framework**: React 18 with Vite
- **State Management**: Zustand
- **Routing**: React Router 6
- **Styling**: TailwindCSS
- **Kaspa Integration**: kaspa-wasm SDK

## Key Components

### App.tsx
Main application component with routing.

### Pages
- `LandingPage`: Entry point
- `LobbyPage`: Match browsing and creation
- `CreateMatchPage`: New match setup
- `MatchPage`: Active match interface
- `WalletPage`: Wallet management
- `HistoryPage`: Match history
- `ProfilePage`: User profile
- `EscrowPage`: Deposit interface

### Hooks
- `useKaspaInit`: Initializes WASM Kaspa client
- `useWallet`: Wallet state management
- `useBalance`: Balance tracking
- `useAuthStore`: User authentication
- `useLobbyStore`: Match lobby state

## Project Structure

```
battle-frontend/
├── src/
│   ├── App.tsx
│   ├── main.tsx
│   ├── components/
│   ├── pages/
│   ├── stores/
│   ├── hooks/
│   ├── api/
│   └── config/
├── package.json
├── vite.config.ts
└── tailwind.config.js
```

## Running Locally

1. `cd battle-frontend`
2. `npm install`
3. `npm run dev`

For build and deployment, see [Build & Deployment](build.md).
For testing, see [Tests & Quality](tests.md).
