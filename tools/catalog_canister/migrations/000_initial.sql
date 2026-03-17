CREATE TABLE IF NOT EXISTS sources (
    source_id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    domain TEXT NOT NULL,
    trust TEXT NOT NULL,
    retrieved_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS source_aliases (
    source_id TEXT NOT NULL,
    alias TEXT NOT NULL,
    PRIMARY KEY (source_id, alias),
    FOREIGN KEY (source_id) REFERENCES sources(source_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS source_canisters (
    source_id TEXT NOT NULL,
    canister_id TEXT NOT NULL,
    PRIMARY KEY (source_id, canister_id),
    FOREIGN KEY (source_id) REFERENCES sources(source_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS source_versions (
    source_id TEXT NOT NULL,
    version TEXT NOT NULL,
    PRIMARY KEY (source_id, version),
    FOREIGN KEY (source_id) REFERENCES sources(source_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS source_citations (
    source_id TEXT NOT NULL,
    citation TEXT NOT NULL,
    PRIMARY KEY (source_id, citation),
    FOREIGN KEY (source_id) REFERENCES sources(source_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_source_aliases_alias ON source_aliases(alias);
CREATE INDEX IF NOT EXISTS idx_sources_domain_trust ON sources(domain, trust);
CREATE INDEX IF NOT EXISTS idx_source_versions_version ON source_versions(version);
