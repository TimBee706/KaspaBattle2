-- Security (audit F-07): store only a SHA-256 hash of each session token.
--
-- Until now `sessions.id` held the raw bearer token, so any read access to the database (backup,
-- SQL injection, log leak) was an instant account takeover. The application now stores
-- lowercase-hex SHA-256(token) and hashes the presented cookie before every lookup.
--
-- Existing rows are converted in place, so currently logged-in users stay logged in. Raw tokens are
-- 43-character base64url strings and hashes are 64 hex characters, so the length predicate makes
-- this statement idempotent (a re-run never hashes a hash).
UPDATE sessions
SET id = encode(sha256(convert_to(id, 'UTF8')), 'hex')
WHERE length(id) <> 64;
