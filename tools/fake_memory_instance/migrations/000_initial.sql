CREATE TABLE IF NOT EXISTS documents (
    id INTEGER PRIMARY KEY,
    payload_json TEXT NOT NULL,
    title TEXT NOT NULL,
    snippet TEXT NOT NULL,
    citation TEXT NOT NULL,
    version TEXT,
    content TEXT NOT NULL,
    embedding_json TEXT NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS documents_fts
USING fts5(search_text, tokenize='trigram');
