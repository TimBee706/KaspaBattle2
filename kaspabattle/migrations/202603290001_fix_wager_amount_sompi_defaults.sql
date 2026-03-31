UPDATE matches
SET wager_amount_sompi = wager_sompi
WHERE wager_amount_sompi IS NULL;

ALTER TABLE matches
ALTER COLUMN wager_amount_sompi SET DEFAULT 0;

ALTER TABLE matches
ALTER COLUMN wager_amount_sompi SET NOT NULL;
