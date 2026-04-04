-- Migration: Add faceit_url to users table
-- Used to store the direct FACEIT profile URL for display on match pages.

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'users' AND column_name = 'faceit_url'
    ) THEN
        ALTER TABLE users ADD COLUMN faceit_url TEXT;
    END IF;
END $$;
