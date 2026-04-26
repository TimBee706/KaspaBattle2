-- Add wager bounds check
ALTER TABLE matches ADD CONSTRAINT check_wager_bounds CHECK (stake_kas > 0);
