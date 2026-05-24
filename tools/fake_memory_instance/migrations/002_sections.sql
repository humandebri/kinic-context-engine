CREATE TABLE IF NOT EXISTS sections (
    id INTEGER PRIMARY KEY,
    scope_key TEXT NOT NULL UNIQUE,
    section_id TEXT NOT NULL,
    title TEXT NOT NULL,
    summary TEXT NOT NULL,
    version TEXT,
    embedding_json TEXT NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS sections_fts
USING fts5(title, summary, tokenize='trigram');
