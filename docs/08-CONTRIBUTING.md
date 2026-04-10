# Contributing Guide

Welcome to KaspaBattle! This guide covers everything you need to start contributing — from development setup to code style, testing, and the PR process.

---

## Table of Contents

- [Development Setup](#development-setup)
- [Code Style](#code-style)
- [Branching Strategy](#branching-strategy)
- [Testing Guidelines](#testing-guidelines)
- [Pull Request Process](#pull-request-process)
- [Issue Templates](#issue-templates)
- [Release Process](#release-process)
- [Community](#community)

---

## Development Setup

### Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| **Rust** | 1.83+ | Backend compilation |
| **Node.js** | 18+ | Frontend build |
| **Docker** | 20+ | Database + services |
| **PostgreSQL** | 15+ | Data persistence (via Docker) |
| **Git** | 2.30+ | Version control |

### Quick Setup

```bash
# 1. Clone the repository
git clone https://github.com/TimBee706/KaspaBattle2.git
cd KaspaBattle2

# 2. Start PostgreSQL
docker-compose up -d postgres

# 3. Backend setup
cd kaspabattle
cp .env.example .env
# Edit .env: set KASPA_NODE_URL to a testnet node
cargo build

# 4. Frontend setup (new terminal)
cd battle-frontend
npm install

# 5. Run
# Terminal 1: cargo run -p battle-api
# Terminal 2: npm run dev
```

### Environment Variables

Copy `.env.example` to `.env` and configure:

| Variable | Required | Description |
|----------|----------|-------------|
| `DATABASE_URL` | ✅ | PostgreSQL connection string |
| `KASPA_NETWORK` | ✅ | `testnet-12` or `mainnet` |
| `KASPA_NODE_URL` | ✅ | wRPC URL to kaspad |
| `FACEIT_CLIENT_ID` | ✅ | FACEIT OAuth client ID |
| `FACEIT_CLIENT_SECRET` | ✅ | FACEIT OAuth client secret |
| `FACEIT_REDIRECT_URI` | ✅ | OAuth callback URL |
| `ESCROW_MNEMONIC` | ✅ | BIP-44 mnemonic for escrow wallet |
| `SESSION_SECRET` | ✅ | Random string for session encryption |

### IDE Setup

**VS Code** (recommended):

- Install `rust-analyzer` extension
- Install `ESLint` extension
- Install `Prettier` extension
- The workspace already includes recommended settings

**IntelliJ/CLion**:

- Install the Rust plugin
- Configure Cargo workspace in `kaspabattle/`

---

## Code Style

### Rust

We use `rustfmt` and `clippy` for consistent code formatting and quality:

```bash
# Format all Rust code
cargo fmt --all

# Run clippy with all warnings
cargo clippy --all-targets --all-features -- -D warnings
```

**Key conventions:**

- Use `snake_case` for functions, variables, and modules
- Use `CamelCase` for types and traits
- Use `SCREAMING_SNAKE_CASE` for constants
- Prefer `Result<T, E>` over panics
- Document public APIs with `///` doc comments
- Keep functions under 50 lines where possible

**Clippy configuration** (from `clippy.toml` in kdapp):

- Default clippy rules apply
- No custom lints configured — standard Rust idioms expected

**Rustfmt configuration** (from `.rustfmt.toml` in kdapp):

- `max_width = 150` (slightly wider than default)
- Other settings at default

### TypeScript / React

We use ESLint with the `@typescript-eslint` plugin:

```bash
cd battle-frontend
npm run lint
```

**Key conventions:**

- Use TypeScript strict mode
- Use functional components with hooks
- Use Zustand for state management (not Redux)
- Use `const` by default, `let` only when needed
- No `any` types — use `unknown` and type narrowing
- Component files use `.tsx`, utility files use `.ts`

### CSS

- Tailwind CSS for utility-first styling
- Custom CSS in `index.css` for global styles and design tokens
- Component-specific styles co-located with components

---

## Branching Strategy

### Branch Naming

| Prefix | Purpose | Example |
|--------|---------|---------|
| `feature/` | New feature | `feature/multi-game-support` |
| `fix/` | Bug fix | `fix/auth-rate-limit` |
| `docs/` | Documentation | `docs/api-reference` |
| `refactor/` | Code refactoring | `refactor/escrow-service` |
| `chore/` | Build/tooling changes | `chore/update-deps` |

### Workflow

```
main (protected)
  │
  ├── feature/my-feature   ← Create branch from main
  │   ├── commit 1
  │   ├── commit 2
  │   └── commit 3
  │       │
  │       └── PR → main    ← Squash merge
  │
  └── main (updated)
```

1. Create a branch from `main`
2. Make your changes with descriptive commits
3. Push and open a Pull Request
4. Address review feedback
5. Squash merge into `main`

---

## Testing Guidelines

### Rust Tests

```bash
# Run all unit tests
cd kaspabattle
cargo test

# Run tests for a specific crate
cargo test -p battle-core
cargo test -p battle-kaspa

# Run a specific test
cargo test test_join_match_success

# Run with output
cargo test -- --nocapture
```

**Test organization:**

| Test Type | Location | Purpose |
|-----------|----------|---------|
| Unit tests | `#[cfg(test)] mod tests` in each file | Test individual functions |
| Integration tests | `kaspabattle/tests/` | Test cross-crate behavior |
| Frontend tests | `battle-frontend/src/__tests__/` | Test React components and flows |

### What to Test

**battle-core:**
- All state machine transitions (valid and invalid)
- Edge cases (same player join, double deposit, etc.)
- Error types and messages

**battle-kaspa:**
- Escrow address derivation (determinism, uniqueness)
- Deposit tracking (none, partial, complete)
- Payout calculation (fee percentages, rounding)
- Use `MockKaspaClient` for RPC mocking

**battle-api:**
- Route handler responses
- Authentication guard
- Input validation

**Frontend:**
- Component rendering
- Store state transitions
- API client error handling

### Frontend Tests

```bash
cd battle-frontend

# Run all tests
npm test

# Watch mode
npm run test:watch

# Coverage report
npm run test:coverage
```

Uses Vitest + Testing Library + jsdom.

### Manual Testing Checklist

Before submitting a PR that affects the match flow:

- [ ] Create a challenge via the UI
- [ ] Join a challenge as a different user
- [ ] Verify escrow address is generated
- [ ] Cancel a challenge and verify state
- [ ] Check WebSocket events fire correctly

---

## Pull Request Process

### Before Opening a PR

1. **Lint**: `cargo clippy --all-targets -- -D warnings`
2. **Format**: `cargo fmt --all --check`
3. **Test**: `cargo test`
4. **Frontend lint**: `cd battle-frontend && npm run lint`
5. **Frontend test**: `cd battle-frontend && npm test`

### PR Template

```markdown
## Description
Brief description of changes.

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Manual testing completed

## Checklist
- [ ] Code follows project style guidelines
- [ ] Self-review completed
- [ ] Documentation updated (if applicable)
- [ ] No new warnings introduced
```

### Review Criteria

PRs are reviewed for:

- **Correctness**: Does it do what it claims?
- **Tests**: Are new features/fixes covered by tests?
- **Style**: Does it follow project conventions?
- **Security**: Are there any security implications?
- **Performance**: Any performance concerns?

---

## Issue Templates

### Bug Report

```markdown
**Describe the bug**
A clear description of the unexpected behavior.

**To Reproduce**
1. Step 1
2. Step 2
3. ...

**Expected behavior**
What should happen instead.

**Environment**
- OS: [e.g., Windows 11, Ubuntu 22.04]
- Rust version: [e.g., 1.83.0]
- Node version: [e.g., 18.19.0]
- Browser: [e.g., Chrome 121]

**Logs**
Relevant error messages or log output.
```

### Feature Request

```markdown
**Is your feature request related to a problem?**
A description of the problem.

**Describe the solution you'd like**
What you want to happen.

**Describe alternatives you've considered**
Other approaches you've thought about.

**Additional context**
Any other context, mockups, or references.
```

---

## Release Process

### Versioning

KaspaBattle follows [Semantic Versioning](https://semver.org/):

- **MAJOR**: Breaking API changes
- **MINOR**: New features (backward compatible)
- **PATCH**: Bug fixes (backward compatible)

### Release Steps

1. Update version in `Cargo.toml` (workspace)
2. Update version in `battle-frontend/package.json`
3. Update `CHANGELOG.md`
4. Create a git tag: `git tag v1.2.3`
5. Push tag: `git push origin v1.2.3`
6. CI builds and publishes Docker images

### Docker Images

Production Docker images are built via CI:

```bash
# Build locally
docker build -t kaspabattle-backend:latest -f kaspabattle/Dockerfile kaspabattle/
docker build -t kaspabattle-frontend:latest -f battle-frontend/Dockerfile battle-frontend/
```

---

## Community

### Communication

- **GitHub Issues**: Bug reports and feature requests
- **GitHub Discussions**: General questions and ideas
- **Pull Requests**: Code contributions

### Code of Conduct

We follow the [Contributor Covenant](https://www.contributor-covenant.org/) code of conduct. Be respectful, constructive, and welcoming to all contributors.

### Related Projects

| Project | Description | Link |
|---------|-------------|------|
| kdapp | Kaspa decentralized app framework | [github.com/michaelsutton/kdapp](https://github.com/michaelsutton/kdapp) |
| rusty-kaspa | Kaspa node implementation | [github.com/kaspanet/rusty-kaspa](https://github.com/kaspanet/rusty-kaspa) |
| Kaspa WASM SDK | Browser wallet SDK | [kaspa.aspectron.org](https://kaspa.aspectron.org) |

---

## Thank You

Every contribution makes KaspaBattle better. Whether it's a bug report, documentation improvement, or new feature — your help is appreciated! 🎮
