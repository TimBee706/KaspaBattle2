# ─── Base: Node 20 ─────────────────────────────────────────────────────────────
FROM node:20-slim AS base
WORKDIR /app
RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*

# ─── Dependencies layer ─────────────────────────────────────────────────────────
FROM base AS deps
# Copy package files
COPY battle-frontend/package*.json ./

# kaspa-wasm is a file: dependency – copy the local wasm package first
COPY rusty-kaspa/wasm/web/kaspa /kaspa-wasm-local

# Patch package.json to reference the copied wasm (in-container absolute path)
RUN sed -i 's|file:../rusty-kaspa/wasm/web/kaspa|file:/kaspa-wasm-local|g' package.json

RUN npm install

# ─── Development target (used by docker-compose default) ───────────────────────
FROM base AS dev
WORKDIR /app

COPY --from=deps /kaspa-wasm-local /kaspa-wasm-local
COPY --from=deps /app/node_modules ./node_modules
COPY battle-frontend/ .
# Keep the patched package.json
COPY --from=deps /app/package.json ./package.json

EXPOSE 5173

HEALTHCHECK --interval=15s --timeout=5s --start-period=20s --retries=3 \
    CMD curl -f http://localhost:5173 || exit 1

# --host exposes to all interfaces so Docker port mapping works
CMD ["npx", "vite", "--host", "0.0.0.0"]

# ─── Production build target ────────────────────────────────────────────────────
FROM deps AS build
COPY battle-frontend/ .
COPY --from=deps /app/package.json ./package.json
RUN npm run build

FROM base AS prod
RUN npm install -g serve
COPY --from=build /app/dist ./dist
EXPOSE 5173
HEALTHCHECK --interval=15s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:5173 || exit 1
CMD ["serve", "-s", "dist", "-l", "5173"]
