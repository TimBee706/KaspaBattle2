-- Migration: Add stats_cache fields to faceit_links for /faceit/stats caching (F-04)
-- Stores the last-fetched FACEIT stats JSON so we can return cached data when
-- the upstream FACEIT Data API is unavailable, instead of returning a 502 error.

ALTER TABLE faceit_links
    ADD COLUMN IF NOT EXISTS stats_cache_json       JSONB,
    ADD COLUMN IF NOT EXISTS stats_cache_game_id    TEXT,
    ADD COLUMN IF NOT EXISTS stats_cache_updated_at TIMESTAMPTZ;
