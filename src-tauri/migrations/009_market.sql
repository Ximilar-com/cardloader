-- A card's active listings on eBay (found by an AI agent or from its detail panel) and the price range of those that
-- are the card: the listings as JSON, and the range apart, small enough for every row of the session table.
ALTER TABLE items ADD COLUMN market TEXT;
ALTER TABLE items ADD COLUMN market_range TEXT;
