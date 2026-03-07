<#
.SYNOPSIS
    KaspaBattle Docker Compose orchestration script.

.DESCRIPTION
    Manages all KaspaBattle services (postgres, kaspa-node, backend, frontend)
    via Docker Compose with a single command.

.PARAMETER Command
    up        - Build images and start all services (detached)
    down      - Stop and remove all containers
    restart   - Restart all (or a specific) service
    logs      - Tail logs for all (or a specific) service
    status    - Show container status
    build     - Rebuild images without cache
    db-migrate - Manually apply SQL migrations against postgres

.EXAMPLE
    .\start.ps1 up
    .\start.ps1 logs backend
    .\start.ps1 restart kaspa-node
    .\start.ps1 down

#>

param(
    [Parameter(Mandatory=$true, Position=0)]
    [ValidateSet("up","down","restart","logs","status","build","db-migrate")]
    [string]$Command,

    [Parameter(Position=1)]
    [string]$Service = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# ── Helpers ─────────────────────────────────────────────────────────────────────
$ComposeFile = "docker-compose.yml"
$EnvFile     = ".env.docker"
$BaseArgs    = @("--env-file", $EnvFile, "-f", $ComposeFile)

function Write-Header([string]$msg) {
    Write-Host ""
    Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor DarkCyan
    Write-Host "  🎮 KaspaBattle · $msg" -ForegroundColor Cyan
    Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor DarkCyan
    Write-Host ""
}

function Assert-Docker {
    if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
        Write-Host "❌  Docker not found. Please install Docker Desktop." -ForegroundColor Red
        exit 1
    }
    $info = docker info 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "❌  Docker daemon is not running. Please start Docker Desktop." -ForegroundColor Red
        exit 1
    }
}

function Assert-EnvFile {
    if (-not (Test-Path $EnvFile)) {
        Write-Host "⚠️  $EnvFile not found — copying from .env as template..." -ForegroundColor Yellow
        if (Test-Path ".env") {
            Copy-Item ".env" $EnvFile
            Write-Host "   ✅ Copied .env → $EnvFile" -ForegroundColor Green
            Write-Host "   ℹ️  Review $EnvFile and ensure DATABASE_URL/KASPA_NODE_URL use service names." -ForegroundColor Gray
        } else {
            Write-Host "❌  Neither .env.docker nor .env found. Cannot continue." -ForegroundColor Red
            exit 1
        }
    }
}

# ── Commands ─────────────────────────────────────────────────────────────────────

function Invoke-Up {
    Write-Header "Starting all services"
    Write-Host "🔨 Building images (this may take a few minutes on first run)..." -ForegroundColor Yellow
    docker compose @BaseArgs up --build --detach
    if ($LASTEXITCODE -ne 0) { Write-Host "❌ Compose up failed." -ForegroundColor Red; exit 1 }
    Write-Host ""
    Write-Host "✅ All services started!" -ForegroundColor Green
    Write-Host ""
    Write-Host "   Frontend  → http://localhost:5173" -ForegroundColor White
    Write-Host "   Backend   → http://localhost:8080/api/v1/lobbies" -ForegroundColor White
    Write-Host "   Kaspa RPC → ws://localhost:16111 (wRPC Borsh)" -ForegroundColor White
    Write-Host "   Postgres  → localhost:5432 (user: postgres)" -ForegroundColor White
    Write-Host ""
    Write-Host "   Tip: run '.\start.ps1 status' to check health" -ForegroundColor Gray
}

function Invoke-Down {
    Write-Header "Stopping all services"
    docker compose @BaseArgs down
    if ($LASTEXITCODE -ne 0) { Write-Host "❌ Compose down failed." -ForegroundColor Red; exit 1 }
    Write-Host "✅ All containers stopped." -ForegroundColor Green
    Write-Host "   (Volumes are preserved. Use 'docker compose down -v' to delete data.)" -ForegroundColor Gray
}

function Invoke-Build {
    Write-Header "Rebuilding images (no cache)"
    docker compose @BaseArgs build --no-cache
    if ($LASTEXITCODE -ne 0) { Write-Host "❌ Build failed." -ForegroundColor Red; exit 1 }
    Write-Host "✅ Images rebuilt." -ForegroundColor Green
}

function Invoke-Restart([string]$svc) {
    if ($svc) {
        Write-Header "Restarting service: $svc"
        docker compose @BaseArgs restart $svc
    } else {
        Write-Header "Restarting all services"
        docker compose @BaseArgs restart
    }
    if ($LASTEXITCODE -ne 0) { Write-Host "❌ Restart failed." -ForegroundColor Red; exit 1 }
    Write-Host "✅ Done." -ForegroundColor Green
}

function Invoke-Logs([string]$svc) {
    if ($svc) {
        Write-Header "Logs: $svc (Ctrl+C to exit)"
        docker compose @BaseArgs logs -f --tail=100 $svc
    } else {
        Write-Header "Logs: all services (Ctrl+C to exit)"
        docker compose @BaseArgs logs -f --tail=50
    }
}

function Invoke-Status {
    Write-Header "Service Status"
    docker compose @BaseArgs ps --format "table {{.Service}}\t{{.Status}}\t{{.Ports}}"
}

function Invoke-DbMigrate {
    Write-Header "Running DB migrations"
    $migrationsDir = "kaspabattle\migrations"
    $sqlFiles = Get-ChildItem -Path $migrationsDir -Filter "*.sql" | Sort-Object Name

    foreach ($file in $sqlFiles) {
        Write-Host "  📄 Applying $($file.Name) ..." -ForegroundColor Yellow
        $content = Get-Content $file.FullName -Raw
        echo $content | docker compose @BaseArgs exec -T postgres `
            psql -U postgres -d kaspabattle
        if ($LASTEXITCODE -ne 0) {
            Write-Host "     ⚠️  $($file.Name) failed (may already be applied, continuing...)" -ForegroundColor DarkYellow
        } else {
            Write-Host "     ✅ Applied." -ForegroundColor Green
        }
    }
    Write-Host ""
    Write-Host "✅ Migration run complete." -ForegroundColor Green
}

# ── Main ──────────────────────────────────────────────────────────────────────────
Assert-Docker
Assert-EnvFile

switch ($Command) {
    "up"         { Invoke-Up }
    "down"       { Invoke-Down }
    "build"      { Invoke-Build }
    "restart"    { Invoke-Restart $Service }
    "logs"       { Invoke-Logs $Service }
    "status"     { Invoke-Status }
    "db-migrate" { Invoke-DbMigrate }
}
