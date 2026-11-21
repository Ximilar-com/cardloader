-- A card's async card-grader jobs (grading, crop), each with its own state: one that failed keeps why, while the
-- others still out are polled and stored. The card's status and error follow from them (`Db::settle`), so a failed
-- crop no longer hides a grading that is still to come.
CREATE TABLE item_jobs (
    item_id INTEGER NOT NULL REFERENCES items (id) ON DELETE CASCADE,
    kind    TEXT NOT NULL,
    -- NULL: the job was never submitted (the call failed).
    job_id  TEXT,
    -- Why it failed; NULL while it is out.
    error   TEXT,
    PRIMARY KEY (item_id, kind)
);

INSERT INTO item_jobs (item_id, kind, job_id)
    SELECT id, 'grading', grading_job_id FROM items WHERE status = 'grading' AND grading_job_id IS NOT NULL;
INSERT INTO item_jobs (item_id, kind, job_id)
    SELECT id, 'crop', crop_job_id FROM items WHERE status = 'grading' AND crop_job_id IS NOT NULL;

-- `grading_job_id` stays (unused), so that an older version of the app still opens the database; `crop_job_id` was
-- never in a release.
UPDATE items SET grading_job_id = NULL;
ALTER TABLE items DROP COLUMN crop_job_id;

-- The stage of its pass (identify, grade, crop) a card still owes when the pass was cut short (a stop, no
-- connection, the app quit): the next run goes on there instead of identifying the card again.
ALTER TABLE items ADD COLUMN stage TEXT;
