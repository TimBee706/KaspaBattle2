# ─── Base: Node 20 ─────────────────────────────────────────────────────────────
FROM node:20-slim AS base
WORKDIR /app
RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*

# ─── Dependencies layer ─────────────────────────────────────────────────────────
FROM base AS deps
# Copy package files
COPY battle-frontend/package*.json ./

# Copy the vendor directory which contains kaspa-wasm
COPY battle-frontend/vendor ./vendor

RUN npm install

# ─── Development target (used by docker-compose default) ───────────────────────
FROM base AS dev
WORKDIR /app

COPY --from=deps /app/vendor ./vendor
COPY --from=deps /app/node_modules ./node_modules
COPY battle-frontend/ .
COPY --from=deps /app/package.json ./package.json

EXPOSE 5173

HEALTHCHECK --interval=15s --timeout=5s --start-period=20s --retries=3 \
    CMD curl -f http://localhost:5173 || exit 1

# --host exposes to all interfaces so Docker port mapping works
CMD ["npx", "vite", "--host", "0.0.0.0"]

# ─── Production build target ────────────────────────────────────────────────────
# Vite inlines VITE_* into the static bundle at build time (not at container
# start), so these must arrive as Docker build ARGs, not just runtime env vars.
# docker-compose.yml passes them through frontend.build.args for the prod target.
FROM deps AS build
ARG VITE_API_BASE_URL
ARG VITE_WS_BASE_URL
ARG VITE_KASPA_NETWORK
ARG VITE_KASPA_NODE_URL
ENV VITE_API_BASE_URL=$VITE_API_BASE_URL \
    VITE_WS_BASE_URL=$VITE_WS_BASE_URL \
    VITE_KASPA_NETWORK=$VITE_KASPA_NETWORK \
    VITE_KASPA_NODE_URL=$VITE_KASPA_NODE_URL
COPY battle-frontend/ .
COPY --from=deps /app/package.json ./package.json
RUN npm run build

FROM base AS prod
RUN addgroup --system --gid 1001 appgroup && \
    adduser --system --uid 1001 appuser && \
    npm install -g serve
COPY --from=build /app/dist ./dist
USER appuser
EXPOSE 5173
HEALTHCHECK --interval=15s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:5173 || exit 1
CMD ["serve", "-s", "dist", "-l", "5173"]
