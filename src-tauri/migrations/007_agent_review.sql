-- Cards an AI agent accepted, picked or corrected (over MCP) rather than the user, and what it said about them.
-- The user accepting such a card again, or a new recognition, clears both.
ALTER TABLE items ADD COLUMN agent_reviewed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE items ADD COLUMN review_note TEXT;
