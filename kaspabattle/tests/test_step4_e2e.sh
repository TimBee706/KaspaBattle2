#!/bin/bash
# test_step4_e2e.sh

# Variablen
API_URL="http://localhost:3000/api/v1"
EMAIL="testuser_$(date +%s)@example.com"
PASSWORD="SecurePassword123!"
DISPLAY_NAME="TestPlayer"

echo "=== KaspaBattle Step 4 E2E Test ==="

# 1. User Registrieren
echo -e "\n1. Register User..."
RESPONSE=$(curl -s -X POST $API_URL/auth/register \
  -H "Content-Type: application/json" \
  -d "{\"email\":\"$EMAIL\",\"password\":\"$PASSWORD\",\"display_name\":\"$DISPLAY_NAME\"}")
echo $RESPONSE

USER_ID=$(echo $RESPONSE | grep -o '"id":"[^"]*' | cut -d '"' -f 4)
TOKEN=$(echo $RESPONSE | grep -o '"token":"[^"]*' | cut -d '"' -f 4)

if [ -z "$TOKEN" ]; then
    echo "Registrierung fehlgeschlagen!"
    exit 1
fi

echo -e "\nToken erhalten: $TOKEN"

# 2. Get Me
echo -e "\n2. Get User Profile (/me)..."
curl -s -X GET $API_URL/auth/me \
  -H "Authorization: Bearer $TOKEN" | grep -o '"email":"[^"]*'

# 3. Set Kaspa Address
echo -e "\n\n3. Set Kaspa Address..."
curl -s -X PUT $API_URL/auth/me/kaspa-address \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"kaspa_address\":\"kaspatest:qz7...test\"}"

# 4. Init FACEIT Link
echo -e "\n\n4. Init FACEIT OAuth Link..."
LINK_URL=$(curl -i -s -X GET $API_URL/faceit/link \
  -H "Authorization: Bearer $TOKEN" | grep -i "location:")
echo $LINK_URL

# 5. Create Oracle Job
echo -e "\n5. Create Oracle Job..."
curl -s -X POST $API_URL/oracle/jobs \
  -H "Content-Type: application/json" \
  -d "{\"match_id\":\"match-123\",\"faceit_match_id\":\"faceit-456\"}"

# 6. Logout
echo -e "\n\n6. Logout User..."
curl -s -X POST $API_URL/auth/logout \
  -H "Authorization: Bearer $TOKEN"

echo -e "\n\n=== E2E Test Abgeschlossen ==="
