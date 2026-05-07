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

- [x] `src/models/job.rs` — add `WarmupInput` + `TritonDataType`; extend `SubmitJobRequest` and `ConversionJob` with `warmup_inputs: Vec<WarmupInput>`
- [x] `migrations/0006_add_warmup_inputs.sql` — `ALTER TABLE conversion_jobs ADD COLUMN warmup_inputs TEXT NOT NULL DEFAULT '[]'`
- [x] `src/server/db.rs` — round-trip `warmup_inputs` through `insert_job` / `get_job` / `list_*`
- [x] `src/server/onnx_config.rs` — add `format_warmup_blocks`; substitute `$INPUT_WARMUP_BLOCKS` in `fill_template`; strip the entire `model_warmup { … }` block when the vec is empty
- [x] Unit tests for non-empty + empty warmup rendering
- [x] `src/server/conversion.rs` — pass `warmup_inputs` through to `generate_config_pbtxt`
- [x] `src/components/upload_form.rs` — repeatable warmup-input UI with key / data_type / dims / zero_data
- [x] `cargo fmt && cargo clippy -- -D warnings && cargo test` clean
- [x] Commit: `feat(warmup): support multiple model_warmup inputs end-to-end`

## Phase 3 — Start/Stop tritonserver from group card

Each `GroupCard` exposes a Start (▶) / Stop (■) icon and a status pill, plus an expandable Logs panel that streams live container output (2 s polling, mirrors job_detail).

- [x] `migrations/0007_create_tritonserver_containers.sql` — `tritonserver_containers` + `tritonserver_logs` tables
- [x] `src/models/serving.rs` — `ServingContainer` struct + `ServingStatus` enum
- [x] `src/server/db.rs` — `upsert_serving_container`, `get_serving_by_group`, `update_serving_status`, `append_serving_logs_batch`, `tail_serving_logs`
- [x] `src/server/serving.rs` — `start_tritonserver` (binds group dir to `/models`, exposes 8000/8001/8002, GPU device request), `stop_tritonserver`, `spawn_log_pump`
- [x] `src/api.rs` — `start_group_serving(group_id, gpu_id)`, `stop_group_serving(group_id)`, `get_group_serving_status(group_id)`, `get_group_serving_logs(group_id, limit)`
- [x] `src/components/group_card.rs` — status pill, ▶/■ icon, Start dialog with GPU dropdown, expandable `<pre>` logs panel
- [x] `cargo fmt && cargo clippy -- -D warnings && cargo test` clean
- [x] Commit: `feat(groups): start/stop tritonserver from group card with live logs`

## Phase 4 — UX refinements

Post-launch UX cleanup: warmup is auto-derived from ONNX (no user input), config.pbtxt editor is large with horizontal scroll, group-card buttons line up, and the Start dialog + Logs panel render full-width below the grid instead of growing the card.

### 4a — Auto-generate warmup from ONNX
- [x] `src/server/onnx_config.rs` — drop `warmup_inputs` parameter; auto-derive warmup entries from `metadata.inputs` inside `fill_template`; remove `strip_model_warmup_block`
- [x] `src/models/job.rs` — delete `WarmupInput` + `TritonDataType`; remove `warmup_inputs` field from `ConversionJob` and `SubmitJobRequest`
- [x] `src/server/conversion.rs` — drop `&job.warmup_inputs` arg
- [x] `src/api.rs` — drop `warmup_inputs` from `build_new_job` and `submit_job`
- [x] `src/server/db.rs` — remove `warmup_inputs` column round-trip
- [x] `migrations/0008_drop_warmup_inputs.sql` — `ALTER TABLE conversion_jobs DROP COLUMN warmup_inputs`
- [x] `src/components/upload_form.rs` — rip out `WarmupDraft` + warmup form section
- [x] Replace warmup tests with `auto_generates_warmup_from_inputs`
- [x] Commit: `refactor(warmup): auto-generate model_warmup from ONNX inputs`

### 4b — Larger, scroll-x config.pbtxt editor
- [x] `src/routes/job_detail.rs` — textarea `h-[70vh] min-h-96 overflow-auto`, `wrap="off"`
- [x] Commit: `style(job_detail): enlarge config.pbtxt editor with horizontal scroll`

### 4c — Equalize group-card action button heights
- [x] `src/components/group_card.rs` — Delete / Confirm buttons get `h-8 inline-flex items-center justify-center`, drop `py-1.5`
- [x] Commit: `style(group_card): equalize action button heights`

### 4d — Lift Start dialog and logs panel out of the card; full-width below grid
- [ ] `src/components/group_card.rs` — remove `show_start_dialog` / `show_logs` / `start_gpu` state; props gain `serving_view`, `on_request_start`, `on_toggle_logs`
- [ ] `src/routes/groups.rs` — `ServingView` enum + signal; render Start dialog and Logs panel full-width below the grid
- [ ] Commit: `feat(groups): render serving start dialog and logs full-width below grid`
