-- Details of a card a listing title can use (who made it, the team, the copy's serial number, what a seller says
-- about it), and the title someone wrote for this card (NULL = the one its template makes).
ALTER TABLE items ADD COLUMN brand TEXT;
ALTER TABLE items ADD COLUMN team TEXT;
ALTER TABLE items ADD COLUMN serial TEXT;
ALTER TABLE items ADD COLUMN features TEXT;
ALTER TABLE items ADD COLUMN title TEXT;
