-- The currency of a card's price (ISO 4217, e.g. EUR from ebay.de); NULL is US dollars, as every price was before.
ALTER TABLE items ADD COLUMN price_currency TEXT;
