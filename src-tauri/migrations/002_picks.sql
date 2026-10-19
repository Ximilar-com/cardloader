-- The user can pick another candidate as the best match. `picks` counts the picks, `picks_synced` the
-- ones cardstream has (they reach it as a PATCH of the row, whose id is `remote_id`).
ALTER TABLE items ADD COLUMN picks INTEGER NOT NULL DEFAULT 0;
ALTER TABLE items ADD COLUMN picks_synced INTEGER NOT NULL DEFAULT 0;
ALTER TABLE items ADD COLUMN remote_id TEXT;
