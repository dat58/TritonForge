CREATE TABLE IF NOT EXISTS conversion_jobs (
    id               TEXT    PRIMARY KEY NOT NULL,
    model_name       TEXT    NOT NULL,
    model_format     TEXT    NOT NULL,
    image_tag        TEXT    NOT NULL,
    gpu_id           INTEGER NOT NULL,
    template_name    TEXT    NOT NULL,
    status           TEXT    NOT NULL DEFAULT 'pending',
    progress_percent INTEGER NOT NULL DEFAULT 0,
    output_path      TEXT,
    error_message    TEXT,
    created_at       TEXT    NOT NULL,
    updated_at       TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_jobs_status     ON conversion_jobs (status);
CREATE INDEX IF NOT EXISTS idx_jobs_created_at ON conversion_jobs (created_at DESC);
