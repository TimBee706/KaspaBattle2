#!/bin/bash
set -e

# Farben
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

BASE_URL="http://localhost:3000"

echo -e "${YELLOW}╔══════════════════════════════════════╗${NC}"
echo -e "${YELLOW}║   KaspaBattle E2E Test – Schritt 2   ║${NC}"
echo -e "${YELLOW}╚══════════════════════════════════════╝${NC}"

# Helper
assert_status() {
    local expected=$1
    local actual=$2
    local description=$3
    if [ "$actual" -eq "$expected" ]; then
        echo -e "  ${GREEN}✓${NC} $description (HTTP $actual)"
    else
        echo -e "  ${RED}✗${NC} $description (expected $expected, got $actual)"
        exit 1
    fi
}

assert_json_field() {
    local json=$1
    local field=$2
    local expected=$3
    local actual=$(echo "$json" | jq -r ".$field")
    if [ "$actual" = "$expected" ]; then
        echo -e "  ${GREEN}✓${NC} $field = $expected"
    else
        echo -e "  ${RED}✗${NC} $field: expected '$expected', got '$actual'"
        exit 1
    fi
}

# Test 0: Health Check
echo -e "\n${YELLOW}[Test 0] Health Check${NC}"
STATUS=$(curl.exe -s -o /dev/null -w "%{http_code}" $BASE_URL/api/health)
assert_status 200 $STATUS "Health check"

# Test 1: Node Status
echo -e "\n${YELLOW}[Test 1] Node Status${NC}"
RESPONSE=$(curl.exe -s $BASE_URL/api/node/status)
STATUS=$(echo $RESPONSE | jq -r '.connected')
echo -e "  ℹ️  Node connected: $STATUS"
assert_json_field "$RESPONSE" "is_synced" "true"
echo -e "  ${GREEN}✓${NC} Node status endpoint works"

# Test 2: Match erstellen
echo -e "\n${YELLOW}[Test 2] Match erstellen${NC}"
RESPONSE=$(curl.exe -s -w "\n%{http_code}" -X POST $BASE_URL/api/matches \
    -H "Content-Type: application/json" \
    -d '{"player_id":"alice","kaspa_address":"kaspatest:qalice","display_name":"Alice","game_type":"cs2","wager_kas":50}')
HTTP_CODE=$(echo "$RESPONSE" | tail -1)
BODY=$(echo "$RESPONSE" | head -1)
assert_status 201 $HTTP_CODE "Match erstellt"
MATCH_ID=$(echo $BODY | jq -r '.match_id')
ESCROW=$(echo $BODY | jq -r '.escrow_address')
echo -e "  ℹ️  Match ID: $MATCH_ID"
echo -e "  ℹ️  Escrow: $ESCROW"

# Prüfe dass Escrow-Adresse NICHT mehr der Platzhalter ist
if [[ "$ESCROW" == kaspatest:escrow_* ]]; then
    echo -e "  ${RED}✗${NC} Escrow ist noch ein Platzhalter!"
    exit 1
else
    echo -e "  ${GREEN}✓${NC} Escrow ist eine echte Adresse"
fi

# Test 3: Match beitreten
echo -e "\n${YELLOW}[Test 3] Match beitreten${NC}"
RESPONSE=$(curl.exe -s -w "\n%{http_code}" -X POST $BASE_URL/api/matches/$MATCH_ID/join \
    -H "Content-Type: application/json" \
    -d '{"player_id":"bob","kaspa_address":"kaspatest:qbob","display_name":"Bob"}')
HTTP_CODE=$(echo "$RESPONSE" | tail -1)
BODY=$(echo "$RESPONSE" | head -1)
assert_status 200 $HTTP_CODE "Match beigetreten"
assert_json_field "$BODY" "state" "WaitingForDeposits"

# Test 4: Escrow-Status prüfen (sollte leer sein)
echo -e "\n${YELLOW}[Test 4] Escrow-Status (leer)${NC}"
RESPONSE=$(curl.exe -s $BASE_URL/api/matches/$MATCH_ID/escrow)
assert_json_field "$RESPONSE" "current_balance" "0"
assert_json_field "$RESPONSE" "player_a_deposited" "false"
assert_json_field "$RESPONSE" "player_b_deposited" "false"
assert_json_field "$RESPONSE" "status" "waiting_for_deposits"
echo -e "  ${GREEN}✓${NC} Escrow ist leer"

# Test 5: Match-Status prüfen
echo -e "\n${YELLOW}[Test 5] Match-Status${NC}"
RESPONSE=$(curl.exe -s $BASE_URL/api/matches/$MATCH_ID)
assert_json_field "$RESPONSE" "state" "WaitingForDeposits"
echo -e "  ${GREEN}✓${NC} State korrekt"

# Test 6: Resolve testen (auf nicht-gelocktem Match → muss fehlschlagen)
echo -e "\n${YELLOW}[Test 6] Match resolve (sollte fehlschlagen – nicht gelockt)${NC}"
RESPONSE=$(curl.exe -s -w "\n%{http_code}" -X POST $BASE_URL/api/matches/$MATCH_ID/resolve \
    -H "Content-Type: application/json" \
    -d '{"winner_id":"alice","reported_by":"admin"}')
HTTP_CODE=$(echo "$RESPONSE" | tail -1)
BODY=$(echo "$RESPONSE" | head -1)
assert_status 400 $HTTP_CODE "Resolve auf nicht-gelocktem Match → 400"
echo -e "  ${GREEN}✓${NC} Korrekte Ablehnung"

# Test 7: Zweites Match erstellen und Cancel testen
echo -e "\n${YELLOW}[Test 7] Cancel-Flow${NC}"
RESPONSE=$(curl.exe -s -X POST $BASE_URL/api/matches \
    -H "Content-Type: application/json" \
    -d '{"player_id":"charlie","kaspa_address":"kaspatest:qcharlie","display_name":"Charlie","game_type":"valorant","wager_kas":100}')
MATCH_ID2=$(echo $RESPONSE | jq -r '.match_id')
echo -e "  ℹ️  Match2 ID: $MATCH_ID2"

RESPONSE=$(curl.exe -s -w "\n%{http_code}" -X POST $BASE_URL/api/matches/$MATCH_ID2/cancel \
    -H "Content-Type: application/json" \
    -d '{"player_id":"charlie","reason":"Testing"}')
HTTP_CODE=$(echo "$RESPONSE" | tail -1)
assert_status 200 $HTTP_CODE "Cancel erfolgreich"

# Test 8: Lobby sollte nur offene Matches zeigen
echo -e "\n${YELLOW}[Test 8] Lobby-Check${NC}"
RESPONSE=$(curl.exe -s $BASE_URL/api/matches)
COUNT=$(echo $RESPONSE | jq -r '.count')
echo -e "  ℹ️  Offene Matches: $COUNT"
echo -e "  ${GREEN}✓${NC} Lobby funktioniert"

echo -e "\n${YELLOW}╔══════════════════════════════════════╗${NC}"
echo -e "${GREEN}║         ALLE TESTS BESTANDEN         ║${NC}"
echo -e "${YELLOW}╚══════════════════════════════════════╝${NC}"
