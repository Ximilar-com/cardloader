-- A card's eBay listing (`ebay::sync`): its SKU, the offer (unpublished = a draft), the listing once published,
-- the price it was offered at, the pictures uploaded to eBay (a JSON array of URLs) and the last error.
-- `ebay_status`: NULL, 'draft', 'listed', 'sold' or 'ended'.
ALTER TABLE items ADD COLUMN ebay_sku TEXT;
ALTER TABLE items ADD COLUMN ebay_offer_id TEXT;
ALTER TABLE items ADD COLUMN ebay_listing_id TEXT;
ALTER TABLE items ADD COLUMN ebay_status TEXT;
ALTER TABLE items ADD COLUMN ebay_price REAL;
ALTER TABLE items ADD COLUMN ebay_images TEXT;
ALTER TABLE items ADD COLUMN ebay_error TEXT;
-- Why the last eBay job of the session stopped, and when its listings were last checked.
ALTER TABLE sessions ADD COLUMN ebay_error TEXT;
ALTER TABLE sessions ADD COLUMN ebay_checked_at TEXT;
