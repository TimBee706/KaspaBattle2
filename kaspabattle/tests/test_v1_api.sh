#!/bin/bash
# Test script for KaspaBattle V1 API Endpoints (Real Escrow Lifecycle)

BASE_URL="http://localhost:3000"
CHALLENGE_ID="test-challenge-$(date +%s)"

echo "--- 1. Checking Node Status ---"
curl -s "$BASE_URL/api/node/status" | jq .

echo -e "\n--- 2. Creating a Challenge (Existing Match Flow) ---"
# Note: This assumes a match with this ID already exists or we use an existing one
# For testing, we might need to create a match first via /api/matches
MATCH_DATA='{"game_type":"NumberGuess","wager_kas":1.0,"player_a_id":"alice","player_a_addr":"kaspatest:qz7..."}'
# echo $MATCH_DATA | curl -s -X POST -H "Content-Type: application/json" -d @- "$BASE_URL/api/matches"

echo -e "\n--- 3. Creating Escrow for Challenge: $CHALLENGE_ID ---"
# We'll use a known match if available, or just test the endpoint logic
curl -s -X POST "$BASE_URL/api/v1/challenges/$CHALLENGE_ID/escrow" | jq .

echo -e "\n--- 4. Checking Deposit Status ---"
curl -s "$BASE_URL/api/v1/challenges/$CHALLENGE_ID/deposits" | jq .

echo -e "\n--- 5. Cancelling Challenge (Simulate Refund) ---"
curl -s -X POST -H "Content-Type: application/json" -d '{"reason":"Test cancel"}' "$BASE_URL/api/v1/challenges/$CHALLENGE_ID/cancel" | jq .
