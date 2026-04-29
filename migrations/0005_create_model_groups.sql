CREATE TABLE IF NOT EXISTS model_groups (
    id         TEXT    PRIMARY KEY NOT NULL,
    name       TEXT    NOT NULL UNIQUE,
    dir_path   TEXT    NOT NULL,
    created_at TEXT    NOT NULL,
    updated_at TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_model_groups_created_at ON model_groups (created_at DESC);

CREATE TABLE IF NOT EXISTS model_group_members (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    group_id   TEXT    NOT NULL,
    job_id     TEXT    NOT NULL,
    model_name TEXT    NOT NULL,
    created_at TEXT    NOT NULL,
    FOREIGN KEY (group_id) REFERENCES model_groups (id) ON DELETE CASCADE,
    UNIQUE (group_id, model_name)
);

CREATE INDEX IF NOT EXISTS idx_mgm_group_id ON model_group_members (group_id);
