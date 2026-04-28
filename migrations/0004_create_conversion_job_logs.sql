CREATE TABLE IF NOT EXISTS conversion_job_logs (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id     TEXT    NOT NULL,
    stream     TEXT    NOT NULL,
    message    TEXT    NOT NULL,
    created_at TEXT    NOT NULL,
    FOREIGN KEY (job_id) REFERENCES conversion_jobs (id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_job_logs_job_id ON conversion_job_logs (job_id);
CREATE INDEX IF NOT EXISTS idx_job_logs_job_id_id ON conversion_job_logs (job_id, id);
