-- Native Games (Phase 1b): new lifecycle states for provider NATIVE.
--   FUNDED -> READY_TO_PLAY -> IN_GAME -> FINISHED_GAME -> READY_FOR_PAYOUT
--   Draw    -> REFUND_PENDING -> REFUNDED
-- Separate file: a new enum value cannot be used inside the transaction that adds it.
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_enum WHERE enumtypid = 'match_status'::regtype AND enumlabel = 'READY_TO_PLAY')  THEN ALTER TYPE match_status ADD VALUE 'READY_TO_PLAY';  END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_enum WHERE enumtypid = 'match_status'::regtype AND enumlabel = 'FINISHED_GAME')  THEN ALTER TYPE match_status ADD VALUE 'FINISHED_GAME';  END IF; END $$;
DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_enum WHERE enumtypid = 'match_status'::regtype AND enumlabel = 'REFUND_PENDING') THEN ALTER TYPE match_status ADD VALUE 'REFUND_PENDING'; END IF; END $$;
