ALTER TABLE sources ADD COLUMN skill_kind TEXT;

CREATE TABLE IF NOT EXISTS source_targets (
    source_id TEXT NOT NULL,
    target TEXT NOT NULL,
    PRIMARY KEY (source_id, target),
    FOREIGN KEY (source_id) REFERENCES sources(source_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS source_capabilities (
    source_id TEXT NOT NULL,
    capability TEXT NOT NULL,
    PRIMARY KEY (source_id, capability),
    FOREIGN KEY (source_id) REFERENCES sources(source_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_source_targets_target ON source_targets(target);
CREATE INDEX IF NOT EXISTS idx_source_capabilities_capability ON source_capabilities(capability);
