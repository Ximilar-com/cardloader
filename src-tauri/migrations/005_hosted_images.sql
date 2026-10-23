-- The card's pictures on a public image host (ImgBB), a JSON array of URLs: exports that link to pictures
-- (Whatnot) reuse them instead of uploading again.
ALTER TABLE items ADD COLUMN hosted_images TEXT;
