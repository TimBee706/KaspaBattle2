#!/usr/bin/env bash
#
# KaspaBattle Docker Compose orchestration script.
#
# Commands:
#   up         - Build images and start all Docker services (detached)
#   down       - Stop and remove Docker containers
#   restart    - Restart all (or a specific) Docker service
#   logs       - Tail logs for all (or a specific) Docker service
#   status     - Show Docker container status
#   build      - Rebuild Docker images without cache
#   db-migrate - Apply SQL migrations against postgres
#   local      - Start frontend on 5173 and a mock backend on 8080

set -euo pipefail

COMMAND=${1:-}
SERVICE=${2:-}

COMPOSE_FILE="docker-compose.yml"
ENV_FILE=".env.docker"
BASE_ARGS=("--env-file" "$ENV_FILE" "-f" "$COMPOSE_FILE")

write_header() {
    echo ""
    echo "============================================="
    echo "  KaspaBattle - $1"
    echo "============================================="
    echo ""
}

assert_docker() {
    if ! command -v docker &> /dev/null; then
        echo "Docker not found. Please install Docker Desktop or Engine."
        exit 1
    fi

    if ! docker info &> /dev/null; then
        echo "Docker daemon is not running. Please start Docker."
        exit 1
    fi
}

assert_env_file() {
    if [ ! -f "$ENV_FILE" ]; then
        echo "$ENV_FILE not found - copying from .env as template..."
        if [ -f ".env" ]; then
            cp .env "$ENV_FILE"
            echo "Copied .env -> $ENV_FILE"
            echo "Review $ENV_FILE and ensure DATABASE_URL/KASPA_NODE_URL use service names."
        else
            echo "Neither .env.docker nor .env found. Cannot continue."
            exit 1
        fi
    fi
}

invoke_up() {
    write_header "Starting all Docker services"
    echo "Building images (this may take a few minutes on first run)..."
    docker compose "${BASE_ARGS[@]}" up --build --detach
    
    echo ""
    echo "All services started."
    echo ""
    echo "   Frontend  -> http://localhost:5173"
    echo "   Backend   -> http://localhost:8080/api/v1/lobbies"
    echo "   Kaspa RPC -> ws://localhost:16111"
    echo "   Postgres  -> localhost:5432 (user: postgres)"
    echo ""
    echo "Tip: run './start.sh status' to check health"
}

invoke_down() {
    write_header "Stopping all Docker services"
    docker compose "${BASE_ARGS[@]}" down
    echo "All containers stopped."
    echo "(Volumes are preserved. Use \"docker compose down -v\" to delete data.)"
}

invoke_build() {
    write_header "Rebuilding Docker images"
    docker compose "${BASE_ARGS[@]}" build --no-cache
    echo "Images rebuilt."
}

invoke_restart() {
    if [ -n "$SERVICE" ]; then
        write_header "Restarting Docker service: $SERVICE"
        docker compose "${BASE_ARGS[@]}" restart "$SERVICE"
    else
        write_header "Restarting all Docker services"
        docker compose "${BASE_ARGS[@]}" restart
    fi
    echo "Done."
}

invoke_logs() {
    if [ -n "$SERVICE" ]; then
        write_header "Logs: $SERVICE"
        docker compose "${BASE_ARGS[@]}" logs -f --tail=100 "$SERVICE"
    else
        write_header "Logs: all services"
        docker compose "${BASE_ARGS[@]}" logs -f --tail=50
    fi
}

invoke_status() {
    write_header "Docker service status"
    docker compose "${BASE_ARGS[@]}" ps --format "table {{.Service}}\t{{.Status}}\t{{.Ports}}"
}

invoke_db_migrate() {
    write_header "Running DB migrations"
    for file in kaspabattle/migrations/*.sql; do
        echo "Applying $(basename "$file") ..."
        cat "$file" | docker compose "${BASE_ARGS[@]}" exec -T postgres psql -U postgres -d kaspabattle || echo "$(basename "$file") failed (may already be applied, continuing...)"
        echo "Applied."
    done
    echo ""
    echo "Migration run complete."
}

stop_stale_local_ports() {
    # Attempt to kill processes holding port 5173 and 8080 (basic implementation for macOS/Linux)
    for port in 5173 8080; do
        pid=$(lsof -ti :$port || echo "")
        if [ -n "$pid" ]; then
            echo "Releasing port $port (PID $pid)"
            kill -9 $pid || true
        fi
    done
}

invoke_local() {
    write_header "Starting local frontend + mock backend"
    stop_stale_local_ports

    cd battle-frontend
    npm run dev:mock-backend &
    MOCK_PID=$!
    sleep 2
    npm run dev:strict &
    FRONT_PID=$!

    echo ""
    echo "Local dev mode started."
    echo ""
    echo "   Frontend  -> http://localhost:5173"
    echo "   Backend   -> http://localhost:8080"
    echo "   API       -> http://localhost:8080/api/v1/lobbies"
    echo ""
    echo "Note: 'local' uses a mock backend fallback when Docker/Postgres is unavailable."
    
    # Wait for background jobs
    wait $MOCK_PID $FRONT_PID
}

if [ "$COMMAND" != "local" ]; then
    assert_docker
    assert_env_file
fi

case "$COMMAND" in
    up) invoke_up ;;
    down) invoke_down ;;
    build) invoke_build ;;
    restart) invoke_restart ;;
    logs) invoke_logs ;;
    status) invoke_status ;;
    db-migrate) invoke_db_migrate ;;
    local) invoke_local ;;
    *)
        echo "Usage: $0 {up|down|restart|logs|status|build|db-migrate|local} [service]"
        exit 1
        ;;
esac
