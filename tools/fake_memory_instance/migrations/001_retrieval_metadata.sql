ALTER TABLE documents ADD COLUMN section TEXT;
ALTER TABLE documents ADD COLUMN tags_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE documents ADD COLUMN search_text_primary TEXT NOT NULL DEFAULT '';
ALTER TABLE documents ADD COLUMN search_text_secondary TEXT NOT NULL DEFAULT '';

DROP TABLE IF EXISTS documents_fts;

CREATE VIRTUAL TABLE IF NOT EXISTS documents_fts
USING fts5(primary_text, secondary_text, tags_text, tokenize='trigram');
