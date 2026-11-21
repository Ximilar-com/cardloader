-- A pass of only these stages (comma-separated, e.g. "price,grade"): prices or grading added to cards already
-- identified, without identifying them again. NULL: the session's stages.
ALTER TABLE items ADD COLUMN only_stages TEXT;
