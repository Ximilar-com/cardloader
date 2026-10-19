-- What a graded slab's label says (`recognition::slab_label`), for the table and the CSV export; NULL when the
-- card wasn't in a slab. Cards read before this get it from their stored response.
ALTER TABLE items ADD COLUMN slab_company TEXT;
ALTER TABLE items ADD COLUMN slab_grade TEXT;
ALTER TABLE items ADD COLUMN slab_certificate TEXT;

UPDATE items SET
    slab_company = COALESCE(NULLIF(json_extract(label.value, '$._tags.Company[0].name'), ''),
                            json_extract(label.value, '$._identification.best_match.company'), ''),
    slab_grade = COALESCE(json_extract(label.value, '$._identification.best_match.grade'),
                          json_extract(label.value, '$._tags.Grade[0].name'), ''),
    slab_certificate = COALESCE(json_extract(label.value, '$._identification.best_match.certificate_number'), '')
FROM (SELECT items.id AS item_id, o.value AS value
      FROM items, json_each(items.recognition, '$.response.records[0]._objects') AS o
      WHERE json_extract(o.value, '$.name') = 'Slab Label') AS label
WHERE items.id = label.item_id;
