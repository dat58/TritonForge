# TritonForge — Outstanding Work

Tracking three feature gaps. Each phase ends with a single conventional commit.

## Phase 1 — Edit `config.pbtxt` on completed jobs

Edit (✎) icon next to Download on a completed job. Click → inline editor prefilled with the on-disk pbtxt. Save persists; Cancel closes without writing.

- [x] `src/server/storage.rs` — add `read_config_pbtxt(job_id, model_name)` and `write_config_pbtxt(job_id, model_name, contents)` (atomic write-then-rename)
- [x] `src/api.rs` — add `#[server] get_job_config_pbtxt(job_id)` and `update_job_config_pbtxt(job_id, contents)`, gated on `JobStatus::Completed`, with 256 KiB length cap and `#[tracing::instrument]`
- [x] `src/routes/job_detail.rs` — add ✎ icon button next to Download; toggle inline `<textarea>` editor with Save / Cancel buttons; show errors inline
- [x] `cargo fmt && cargo clippy -- -D warnings && cargo test` clean
- [x] Commit: `feat(job_detail): add inline editor for completed-job config.pbtxt`

## Phase 2 — Multi-input warmup blocks

Upload form collects 0..N warmup inputs `{ key, data_type, dims, zero_data }`. Pipeline persists them and `fill_template()` renders `$INPUT_WARMUP_BLOCKS`. If empty, the whole `model_warmup { … }` block is omitted.

- [ ] `src/models/job.rs` — add `WarmupInput` + `TritonDataType`; extend `SubmitJobRequest` and `ConversionJob` with `warmup_inputs: Vec<WarmupInput>`
- [ ] `migrations/0006_add_warmup_inputs.sql` — `ALTER TABLE conversion_jobs ADD COLUMN warmup_inputs TEXT NOT NULL DEFAULT '[]'`
- [ ] `src/server/db.rs` — round-trip `warmup_inputs` through `insert_job` / `get_job` / `list_*`
- [ ] `src/server/onnx_config.rs` — add `format_warmup_blocks`; substitute `$INPUT_WARMUP_BLOCKS` in `fill_template`; strip the entire `model_warmup { … }` block when the vec is empty
- [ ] Unit tests for non-empty + empty warmup rendering
- [ ] `src/server/conversion.rs` — pass `warmup_inputs` through to `generate_config_pbtxt`
- [ ] `src/components/upload_form.rs` — repeatable warmup-input UI with key / data_type / dims / zero_data
- [ ] `cargo fmt && cargo clippy -- -D warnings && cargo test` clean
- [ ] Commit: `feat(warmup): support multiple model_warmup inputs end-to-end`

## Phase 3 — Start/Stop tritonserver from group card

Each `GroupCard` exposes a Start (▶) / Stop (■) icon and a status pill, plus an expandable Logs panel that streams live container output (2 s polling, mirrors job_detail).

- [ ] `migrations/0007_create_tritonserver_containers.sql` — `tritonserver_containers` + `tritonserver_logs` tables
- [ ] `src/models/serving.rs` — `ServingContainer` struct + `ServingStatus` enum
- [ ] `src/server/db.rs` — `upsert_serving`, `get_serving_by_group`, `mark_serving_stopped`, `append_serving_logs_batch`, `tail_serving_logs`
- [ ] `src/server/docker.rs` — `start_tritonserver` (binds group dir to `/models`, exposes 8000/8001/8002, GPU device request), `stop_tritonserver`, `spawn_tritonserver_log_pump`
- [ ] `src/api.rs` — `start_group_serving(group_id, gpu_id)`, `stop_group_serving(group_id)`, `get_group_serving_status(group_id)`, `get_group_serving_logs(group_id, limit)`
- [ ] `src/components/group_card.rs` — status pill, ▶/■ icon, Start dialog with GPU dropdown, expandable `<pre>` logs panel
- [ ] `cargo fmt && cargo clippy -- -D warnings && cargo test` clean
- [ ] Commit: `feat(groups): start/stop tritonserver from group card with live logs`
