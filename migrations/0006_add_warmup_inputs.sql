ALTER TABLE conversion_jobs
    ADD COLUMN warmup_inputs TEXT NOT NULL DEFAULT '[]';
