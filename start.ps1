<#
.SYNOPSIS
    KaspaBattle Docker Compose orchestration script.

.DESCRIPTION
    Manages KaspaBattle services via Docker Compose and also provides a local
    fallback mode that starts the frontend together with a mock backend.

.PARAMETER Command
    up         - Build images and start all Docker services (detached)
    down       - Stop and remove Docker containers
    restart    - Restart all (or a specific) Docker service
    logs       - Tail logs for all (or a specific) Docker service
    status     - Show Docker container status
    build      - Rebuild Docker images without cache
    db-migrate - Apply SQL migrations against postgres
    local      - Start frontend on 5173 and a mock backend on 8080

.EXAMPLE
    .\start.ps1 up
    .\start.ps1 logs backend
    .\start.ps1 restart kaspa-node
    .\start.ps1 down
    .\start.ps1 local
#>

param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidateSet('up', 'down', 'restart', 'logs', 'status', 'build', 'db-migrate', 'local')]
    [string]$Command,

    [Parameter(Position = 1)]
    [string]$Service = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ComposeFile = 'docker-compose.yml'
$EnvFile = '.env.docker'
$BaseArgs = @('--env-file', $EnvFile, '-f', $ComposeFile)

function Write-Header([string]$Message) {
    Write-Host ''
    Write-Host '=============================================' -ForegroundColor DarkCyan
    Write-Host "  KaspaBattle - $Message" -ForegroundColor Cyan
    Write-Host '=============================================' -ForegroundColor DarkCyan
    Write-Host ''
}

function Assert-Docker {
    if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
        Write-Host 'Docker not found. Please install Docker Desktop.' -ForegroundColor Red
        exit 1
    }

    docker info *> $null
    if ($LASTEXITCODE -ne 0) {
        Write-Host 'Docker daemon is not running. Please start Docker Desktop.' -ForegroundColor Red
        exit 1
    }
}

function Assert-EnvFile {
    if (-not (Test-Path $EnvFile)) {
        Write-Host "$EnvFile not found - copying from .env as template..." -ForegroundColor Yellow
        if (Test-Path '.env') {
            Copy-Item '.env' $EnvFile
            Write-Host "Copied .env -> $EnvFile" -ForegroundColor Green
            Write-Host "Review $EnvFile and ensure DATABASE_URL/KASPA_NODE_URL use service names." -ForegroundColor Gray
        }
        else {
            Write-Host 'Neither .env.docker nor .env found. Cannot continue.' -ForegroundColor Red
            exit 1
        }
    }
}

function Invoke-Up {
    Write-Header 'Starting all Docker services'
    Write-Host 'Building images (this may take a few minutes on first run)...' -ForegroundColor Yellow
    docker compose @BaseArgs up --build --detach
    if ($LASTEXITCODE -ne 0) {
        Write-Host 'Compose up failed.' -ForegroundColor Red
        exit 1
    }

    Write-Host ''
    Write-Host 'All services started.' -ForegroundColor Green
    Write-Host ''
    Write-Host '   Frontend  -> http://localhost:5173' -ForegroundColor White
    Write-Host '   Backend   -> http://localhost:8080/api/v1/lobbies' -ForegroundColor White
    Write-Host '   Kaspa RPC -> ws://localhost:16111' -ForegroundColor White
    Write-Host '   Postgres  -> localhost:5432 (user: postgres)' -ForegroundColor White
    Write-Host ''
    Write-Host "Tip: run '.\start.ps1 status' to check health" -ForegroundColor Gray
}

function Invoke-Down {
    Write-Header 'Stopping all Docker services'
    docker compose @BaseArgs down
    if ($LASTEXITCODE -ne 0) {
        Write-Host 'Compose down failed.' -ForegroundColor Red
        exit 1
    }

    Write-Host 'All containers stopped.' -ForegroundColor Green
    Write-Host '(Volumes are preserved. Use "docker compose down -v" to delete data.)' -ForegroundColor Gray
}

function Invoke-Build {
    Write-Header 'Rebuilding Docker images'
    docker compose @BaseArgs build --no-cache
    if ($LASTEXITCODE -ne 0) {
        Write-Host 'Build failed.' -ForegroundColor Red
        exit 1
    }

    Write-Host 'Images rebuilt.' -ForegroundColor Green
}

function Invoke-Restart([string]$Svc) {
    if ($Svc) {
        Write-Header "Restarting Docker service: $Svc"
        docker compose @BaseArgs restart $Svc
    }
    else {
        Write-Header 'Restarting all Docker services'
        docker compose @BaseArgs restart
    }

    if ($LASTEXITCODE -ne 0) {
        Write-Host 'Restart failed.' -ForegroundColor Red
        exit 1
    }

    Write-Host 'Done.' -ForegroundColor Green
}

function Invoke-Logs([string]$Svc) {
    if ($Svc) {
        Write-Header "Logs: $Svc"
        docker compose @BaseArgs logs -f --tail=100 $Svc
    }
    else {
        Write-Header 'Logs: all services'
        docker compose @BaseArgs logs -f --tail=50
    }
}

function Invoke-Status {
    Write-Header 'Docker service status'
    docker compose @BaseArgs ps --format 'table {{.Service}}\t{{.Status}}\t{{.Ports}}'
}

function Invoke-DbMigrate {
    Write-Header 'Running DB migrations'
    $migrationsDir = 'kaspabattle\migrations'
    $sqlFiles = Get-ChildItem -Path $migrationsDir -Filter '*.sql' | Sort-Object Name

    foreach ($file in $sqlFiles) {
        Write-Host "Applying $($file.Name) ..." -ForegroundColor Yellow
        $content = Get-Content $file.FullName -Raw
        echo $content | docker compose @BaseArgs exec -T postgres psql -U postgres -d kaspabattle

        if ($LASTEXITCODE -ne 0) {
            Write-Host "$($file.Name) failed (may already be applied, continuing...)" -ForegroundColor DarkYellow
        }
        else {
            Write-Host 'Applied.' -ForegroundColor Green
        }
    }

    Write-Host ''
    Write-Host 'Migration run complete.' -ForegroundColor Green
}

function Stop-StaleLocalPorts {
    $ports = @(5173, 8080)
    foreach ($port in $ports) {
        $listeners = Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue
        foreach ($listener in $listeners) {
            $proc = Get-Process -Id $listener.OwningProcess -ErrorAction SilentlyContinue
            if (-not $proc) {
                continue
            }

            if ($proc.ProcessName -in @('wslrelay', 'com.docker.backend', 'node')) {
                Write-Host "Releasing port $port from $($proc.ProcessName) (PID $($proc.Id))" -ForegroundColor Yellow
                Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
            }
        }
    }
}

function Invoke-Local {
    Write-Header 'Starting local frontend + mock backend'
    Stop-StaleLocalPorts

    $frontendDir = Join-Path $PSScriptRoot 'battle-frontend'
    $backendCmd = "Set-Location '$frontendDir'; npm run dev:mock-backend"
    $frontendCmd = "Set-Location '$frontendDir'; npm run dev:strict"

    Start-Process powershell -ArgumentList @('-NoExit', '-Command', $backendCmd) | Out-Null
    Start-Sleep -Seconds 2
    Start-Process powershell -ArgumentList @('-NoExit', '-Command', $frontendCmd) | Out-Null

    Write-Host ''
    Write-Host 'Local dev mode started.' -ForegroundColor Green
    Write-Host ''
    Write-Host '   Frontend  -> http://localhost:5173' -ForegroundColor White
    Write-Host '   Backend   -> http://localhost:8080' -ForegroundColor White
    Write-Host '   API       -> http://localhost:8080/api/v1/lobbies' -ForegroundColor White
    Write-Host ''
    Write-Host "Note: 'local' uses a mock backend fallback when Docker/Postgres is unavailable." -ForegroundColor Gray
}

if ($Command -ne 'local') {
    Assert-Docker
    Assert-EnvFile
}

switch ($Command) {
    'up' { Invoke-Up }
    'down' { Invoke-Down }
    'build' { Invoke-Build }
    'restart' { Invoke-Restart $Service }
    'logs' { Invoke-Logs $Service }
    'status' { Invoke-Status }
    'db-migrate' { Invoke-DbMigrate }
    'local' { Invoke-Local }
}
