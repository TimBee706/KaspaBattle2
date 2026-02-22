-- Migration 006: Add faceit_match_id to matches
ALTER TABLE matches
ADD COLUMN faceit_match_id TEXT;