-- F-REFUND: Add refund tracking fields to matches table
ALTER TABLE matches ADD COLUMN IF NOT EXISTS refund_tx_hash TEXT;
ALTER TABLE matches ADD COLUMN IF NOT EXISTS refund_status TEXT NOT NULL DEFAULT 'none';
ALTER TABLE matches ADD COLUMN IF NOT EXISTS cancelled_at TIMESTAMPTZ;

-- Add REFUNDED enum value to match_status (idempotent)
DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_enum
        WHERE enumtypid = 'match_status'::regtype
          AND enumlabel = 'REFUNDED'
    ) THEN
        ALTER TYPE match_status ADD VALUE 'REFUNDED';
    END IF;
END $$;
