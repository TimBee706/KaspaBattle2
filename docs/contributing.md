# Contributing to KaspaBattle

Thank you for your interest in contributing to KaspaBattle! We welcome help in improving the backend performance, frontend UX, and Kaspa integration.

## Development Setup

The project is a Monorepo. You will need to set up both the backend and frontend for local development.

### Backend (Rust)

1. **CD to workspace**: `cd kaspabattle`
2. **Configure**: Create `battle-api/.env` matching the template in [backend.md](backend.md).
3. **Run**: `cargo run -p battle-api`

### Frontend (React)

1. **CD to workspace**: `cd battle-frontend`
2. **Install**: `npm install`
3. **Run**: `npm run dev`

## Coding Guidelines

### Rust

- **Style**: Always run `cargo fmt` before submitting.
- **Lints**: Check for warnings with `cargo clippy`.
- **Errors**: Prefer `anyhow` for top-level errors and `thiserror` for library (core/kaspa) errors.
- **Async**: Use `tokio` for all asynchronous operations.

### TypeScript / React

- **Types**: Use strict TypeScript. Avoid `any`.
- **Components**: Functional components with Hooks are preferred.
- **Linting**: Run `npm run lint`.

## Testing

### Backend

Run all workspace tests:

```bash
cargo test --workspace
```

### Frontend

Run frontend unit tests:

```bash
npm run test
```

## Creating Pull Requests

1. Fork the repository.
2. Create a feature branch (`git checkout -b feature/amazing-feature`).
3. Commit your changes (`git commit -m 'Add some amazing feature'`).
4. Push to the branch (`git push origin feature/amazing-feature`).
5. Open a Pull Request.

## Security Disclosures

If you discover a security vulnerability, please **do not open a public issue**. Instead, send a detailed report to `security@kaspabattle.io` (or the project maintainer). We will respond within 48 hours to coordinate a fix.

---
[← Back to README](../README.md)
