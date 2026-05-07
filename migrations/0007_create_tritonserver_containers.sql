CREATE TABLE tritonserver_containers (
    group_id TEXT PRIMARY KEY,
    container_id TEXT NOT NULL,
    container_name TEXT NOT NULL,
    image_tag TEXT NOT NULL,
    gpu_id INTEGER NOT NULL,
    status TEXT NOT NULL,
    error_message TEXT,
    started_at TEXT NOT NULL,
    stopped_at TEXT,
    FOREIGN KEY (group_id) REFERENCES model_groups (id) ON DELETE CASCADE
);

CREATE INDEX idx_tritonserver_containers_status ON tritonserver_containers(status);

CREATE TABLE tritonserver_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    container_id TEXT NOT NULL,
    stream TEXT NOT NULL,
    message TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_tritonserver_logs_container ON tritonserver_logs(container_id, id);
