# Build & Deployment

## Development Setup

### Prerequisites
- Rust 1.88.0+
- Node.js 18+
- PostgreSQL 13+
- Docker & Docker Compose

### Backend Setup
```bash
# Clone repository
git clone <repo>
cd kaspabattle

# Setup database
docker run -d --name postgres -e POSTGRES_PASSWORD=password -p 5432:5432 postgres:13

# Build Rust workspace
cargo build

# Run migrations (if any)
# Run API server
cargo run --bin battle-api
```

### Frontend Setup
```bash
cd battle-frontend

# Install dependencies
npm install

# Start dev server
npm run dev
```

### Full Stack with Docker
```bash
# Start all services
docker-compose up

# Build for production
docker-compose -f docker-compose.yml build
```

## Build Configuration

### Rust Workspace
- Uses Cargo workspaces for multi-crate project
- Features: `wasm32-sdk`, `wasm32-keygen`, `heap` profiling
- Targets: native, WASM32

### Frontend Build
- Vite bundler with React plugin
- WASM plugins for Kaspa integration
- TypeScript compilation
- TailwindCSS processing

## Deployment

### Environment Variables
```bash
# Database
DATABASE_URL=postgresql://user:pass@localhost/db

# FACEIT OAuth
FACEIT_CLIENT_ID=...
FACEIT_CLIENT_SECRET=...

# Kaspa RPC
KASPA_RPC_URL=wss://api.kaspa.org

# Admin
ADMIN_TOKEN=secret

# Frontend
FRONTEND_URL=https://app.kaspabattle.com
```

### Production Deployment
1. Build Docker images
2. Deploy to container orchestration (Kubernetes/Docker Swarm)
3. Setup reverse proxy (nginx/Caddy)
4. Configure SSL certificates
5. Setup monitoring (Prometheus/Grafana)

### Kaspa Node Setup
```bash
# Download and run kaspad
./kaspad --utxoindex --rpc

# Or use Docker
docker run -d kaspa/kaspad --utxoindex --rpc
```

## Runtime Configuration

### Node Configuration
- Network: mainnet/testnet/devnet/simnet
- Database: RocksDB presets (default/hdd/ssd)
- RPC: max clients, CORS settings
- Consensus: finality depth, target time
- P2P: inbound/outbound limits

### API Server Config
- Database connection pool
- Session secrets
- CORS origins
- Rate limiting
- Admin tokens

### Frontend Config
- API base URL
- Kaspa network ID
- WebSocket endpoints
- Explorer URLs