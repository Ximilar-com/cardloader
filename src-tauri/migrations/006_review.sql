-- Reviewing a session: `reviewed` is set when the user accepts a card's match (or corrects it), `edited` when
-- they typed its fields or price themselves. Edits reach Cards Cloud like picks do (`picks` counts both).
ALTER TABLE items ADD COLUMN reviewed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE items ADD COLUMN edited INTEGER NOT NULL DEFAULT 0;
-- The user's own notes on a card and where it is kept (a box, a binder page); both are free text.
ALTER TABLE items ADD COLUMN notes TEXT;
ALTER TABLE items ADD COLUMN location TEXT;

-- Files whose cards the user deleted from a session: running the folder again doesn't bring them back.
CREATE TABLE removed_files (
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    PRIMARY KEY (session_id, path)
);

-- Picking a candidate is a review.
UPDATE items SET reviewed = 1 WHERE picks > 0;
