-- A card's crop (Card Grading's crop_level) is a job of its own, next to its grading: a session can identify or grade
-- the cards and also save them cropped. The pictures are files in the session's crop folder.
ALTER TABLE items ADD COLUMN crop_job_id TEXT;
ALTER TABLE items ADD COLUMN crop TEXT;
ALTER TABLE items ADD COLUMN crop_images TEXT;
