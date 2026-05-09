# TensorRT Model Converter

## Project Overview

A fullstack Rust application built with **Dioxus** (backend + frontend) that converts deep learning models from TensorFlow SavedModel and ONNX formats to TensorRT engine format. Users upload models via a web UI, select a TensorRT Docker image version, choose a GPU, pick a `config.pbtxt` template, and submit a conversion job with real-time progress tracking. Completed models can be downloaded locally or saved to a specified server path.

---

## Tech Stack

- **Language:** Rust (edition 2024, toolchain stable 1.89.0)
- **Framework:** Dioxus 0.7+ with fullstack feature (SSR + client hydration)
- **Backend integration:** Axum (via Dioxus fullstack)
- **Async runtime:** Tokio
- **Container orchestration:** Bollard (Docker API client for Rust)
- **Serialization:** serde / serde_json
- **File handling:** tokio::fs, tempfile
- **Database (job state):** SQLite via sqlx (async, compile-time checked queries)
- **Styling:** TailwindCSS (integrated via Dioxus CLI)
- **Build tool:** `dx` (Dioxus CLI)

---

## Project Structure

```
tensorrt-converter/
├── Cargo.toml
├── rust-toolchain.toml              # Pin toolchain: stable 1.89.0
├── Dioxus.toml                      # Dioxus CLI configuration
├── CLAUDE.md                    # This file
├── .claude/
│   ├── settings.json
│   └── commands/
│       ├── review.md
│       └── test.md
├── tailwind.config.js
├── input.css
├── assets/                      # Static assets (icons, images)
├── migrations/                  # SQLx migrations
├── templates/                   # config.pbtxt template files
│   ├── classification.pbtxt
│   ├── detection.pbtxt
│   └── custom/
├── src/
│   ├── main.rs                  # Entry point, dioxus::launch
│   ├── app.rs                   # Root App component + Router
│   ├── routes/
│   │   ├── mod.rs
│   │   ├── home.rs              # Upload + conversion form page
│   │   ├── jobs.rs              # Job list / history page
│   │   └── job_detail.rs        # Single job progress + download page
│   ├── components/
│   │   ├── mod.rs
│   │   ├── upload_form.rs       # Model upload + config selection
│   │   ├── progress_bar.rs      # Real-time conversion progress
│   │   ├── gpu_selector.rs      # GPU device picker
│   │   ├── image_selector.rs    # TensorRT Docker image picker
│   │   ├── template_selector.rs # config.pbtxt template picker
│   │   ├── job_card.rs          # Job summary card
│   │   └── navbar.rs            # Navigation bar
│   ├── server/
│   │   ├── mod.rs
│   │   ├── functions.rs         # #[server] functions (Dioxus server fns)
│   │   ├── conversion.rs        # Core conversion pipeline logic
│   │   ├── docker.rs            # Docker/Bollard operations
│   │   ├── gpu.rs               # GPU detection (nvidia-smi parsing)
│   │   └── storage.rs           # File storage: upload, download, server paths
│   ├── models/
│   │   ├── mod.rs
│   │   ├── job.rs               # ConversionJob struct + status enum
│   │   ├── config.rs            # App configuration (Docker images, paths)
│   │   └── template.rs          # config.pbtxt template model
│   ├── errors/
│   │   ├── mod.rs
│   │   └── app_error.rs         # Unified error type with thiserror
│   └── utils/
│       ├── mod.rs
│       ├── validation.rs        # Input validation (file type, size)
│       └── progress.rs          # Progress calculation helpers
└── tests/
    ├── integration/
    │   ├── conversion_test.rs
    │   ├── docker_test.rs
    │   └── upload_test.rs
    └── unit/
        ├── validation_test.rs
        └── progress_test.rs
```

---

## Architecture Decisions

### Dioxus Fullstack Pattern
- Use `#[server]` attribute for server functions (file upload, job submission, progress polling, download)
- Frontend communicates with backend exclusively through Dioxus server functions — no manual REST endpoints
- Use SSE (Server-Sent Events) or WebSocket via Axum for streaming conversion progress to UI
- Client-side state management via Dioxus signals (`use_signal`, `use_resource`)

### Docker Conversion Pipeline
1. User uploads model file → server saves to temp directory
2. Server spawns a Docker container using selected TensorRT image
3. Model file + config.pbtxt template are bind-mounted into the container
4. Container runs conversion command (trtexec or custom entrypoint)
5. Progress is parsed from container stdout/stderr → streamed to client
6. On completion, output engine file is moved to final destination
7. Container is removed after job finishes

### Job State Machine
```
Pending → Preparing → Converting → Finalizing → Completed
                                                → Failed
```

---

## Coding Standards

### Rust Style & Quality (MANDATORY)

- **Follow Rust 2024 edition idioms** — use `clippy::pedantic` as baseline. Leverage edition 2024 features: `gen` blocks, `use<>` precise capturing in `impl Trait`, unsafe ergonomic improvements, `#[diagnostic]` attributes where helpful
- **No `unwrap()` or `expect()` in production code** — use `?` operator with proper error types. `unwrap()` is ONLY acceptable in tests
- **Unified error handling** — define `AppError` enum with `thiserror`, implement `From` conversions for all external error types. Every module returns `Result<T, AppError>`
- **No code duplication** — extract shared logic into utility functions or traits. If you see the same pattern twice, refactor immediately
- **Low cyclomatic complexity** — no function exceeds 20 lines of logic (excluding struct definitions). Break complex logic into smaller, well-named helper functions
- **Meaningful names** — variables, functions, types must be self-documenting. No single-letter variables except in closures/iterators (`|x|`, `|item|`)
- **Type safety** — use newtypes for domain concepts (e.g., `GpuId(u32)`, `JobId(Uuid)`, `ModelPath(PathBuf)`). Never pass raw strings where a typed wrapper is appropriate
- **Derive macros** — always derive `Debug` on all structs/enums. Derive `Clone`, `Serialize`, `Deserialize` where needed
- **Documentation** — all public functions, structs, and modules must have `///` doc comments explaining purpose and usage. Include `# Examples` in doc comments for complex functions
- **Module organization** — each module file must start with a module-level `//!` doc comment. Re-export public items through `mod.rs`

### Async Rules

- All I/O operations (file, network, Docker API) must be async
- Use `tokio::spawn` for background tasks (conversion jobs)
- Never block the async runtime — no `std::thread::sleep`, use `tokio::time::sleep`
- Use `tokio::sync::mpsc` channels for communication between conversion worker and progress reporter

### Dioxus Component Rules

- Components are pure functions: `fn ComponentName() -> Element`
- Use signals for local state: `let mut state = use_signal(|| initial_value);`
- Use `use_resource` for async data fetching from server functions
- Props must be a separate struct with `#[derive(Props, Clone, PartialEq)]`
- Component files must contain exactly ONE public component. Helper/child components are private
- RSX markup must be clean — no inline business logic. Extract handlers to separate functions
- Use `use_server_future` for data that loads on mount

### File & Upload Handling

- Validate file extension on client side (.pb, .onnx, .savedmodel)
- Validate file size on server side (configurable max, default 2GB)
- Use streaming upload for large files — do not load entire file into memory
- Store uploads in a configurable temp directory with UUID-based naming
- Clean up temp files after job completion or failure (with configurable retention)

### Docker Operations

- Use Bollard crate for all Docker API interactions — never shell out to `docker` CLI
- Always set container resource limits (memory, CPU)
- Always set `--gpus` device constraint matching user-selected GPU
- Set a configurable timeout for conversion jobs (default: 30 minutes)
- Always remove containers after job completion (`AutoRemove: true` or explicit cleanup)
- Pull Docker images lazily on first use, cache image availability in memory

---

## Git Workflow (MANDATORY)

### Build-Gate Rule — ABSOLUTE REQUIREMENT

**NEVER run `git add` or `git commit` unless `cargo clippy -- -D warnings` passes with ZERO warnings and ZERO errors.**

This is the single most important rule. A commit with warnings or build errors is forbidden. Note: `cargo clippy` already performs a full `cargo build` internally — a separate `cargo build` step is redundant and wastes compile time. The full sequence is:

```bash
# Step 1: Format
cargo fmt

# Step 2: Clippy (includes full compilation) — MUST pass with zero warnings/errors
cargo clippy -- -D warnings
# ↳ If ANY warning or error appears → FIX IT FIRST, do NOT proceed to git

# Step 3: Test (if tests exist for changed module)
cargo test

# Step 4: ONLY after steps 1-3 pass cleanly → stage and commit
git add <changed files>
git commit -m "<conventional commit message>"
```

**If `cargo clippy` emits even ONE warning or error** → go back, fix it, re-run `cargo clippy -- -D warnings` until the output is completely clean. Only then proceed to `git add` and `git commit`.

### When to Commit

Commit after every **meaningful, self-contained code change** — not after every single keystroke, but after each logical unit of work:

- ✅ Completed a new struct/enum/trait definition
- ✅ Implemented a new server function
- ✅ Added or updated a UI component
- ✅ Fixed a bug
- ✅ Refactored a module
- ✅ Added tests for a feature
- ✅ Updated configuration or dependencies
- ❌ Do NOT commit half-finished features that don't compile
- ❌ Do NOT commit code that has warnings
- ❌ Do NOT bundle multiple unrelated changes into one commit

### Commit Message Format

Follow Conventional Commits strictly:

```
<type>(<scope>): <short description in imperative mood>

[optional body explaining WHY, not WHAT]
```

**Types:** `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `style`, `perf`

**Scope:** module or component name (e.g., `docker`, `upload_form`, `conversion`, `progress_bar`)

**Examples:**
```bash
git add src/server/docker.rs
git commit -m "feat(docker): add container creation with GPU binding support"

git add src/server/conversion.rs
git commit -m "fix(conversion): handle TensorRT stderr parsing for progress extraction"

git add src/server/functions.rs src/utils/validation.rs
git commit -m "refactor(server): extract file validation into dedicated module"

git add tests/integration/docker_test.rs
git commit -m "test(docker): add integration tests for container lifecycle"
```

### Git Add Rules

- **Always use explicit file paths** in `git add` — NEVER use `git add .` or `git add -A` blindly
- Only stage files that are part of the current logical change
- Review staged files with `git diff --cached` before committing if unsure
- If non-code files changed (Cargo.toml, config, migrations), include them in the same commit only if they are part of the same logical change

### Commit Granularity

- **One logical change per commit** — do NOT bundle unrelated changes
- New struct/enum definitions → separate commit from their usage
- Test additions → separate commit from the feature they test
- Refactors → separate commit, never mixed with feature additions
- Dependency additions in Cargo.toml → commit together with the code that uses them

---

## Build & Run Commands

```bash
# Development
dx serve                           # Start dev server with hot reload
dx serve --platform web            # Explicitly target web platform

# Format & Lint (clippy includes full compilation — no separate cargo build needed)
cargo fmt                          # Format all code
cargo clippy -- -D warnings        # Compile + lint with zero warnings policy
cargo clippy --fix --allow-dirty   # Auto-fix clippy suggestions

# Pre-commit workflow (run this before every git add/commit)
cargo fmt && cargo clippy -- -D warnings && cargo test

# Test
cargo test                         # Run all tests
cargo test --lib                   # Unit tests only
cargo test --test integration      # Integration tests only

# Build for production
dx build --release                 # Optimized build
dx bundle                          # Bundle for deployment

# Database
sqlx migrate run                   # Run pending migrations
sqlx prepare                       # Generate offline query data
```

---

## Environment Configuration

```bash
# .env (DO NOT commit)
DATABASE_URL=sqlite://data/converter.db
UPLOAD_DIR=/tmp/tensorrt-converter/uploads
OUTPUT_DIR=/data/tensorrt-models
MAX_UPLOAD_SIZE_MB=2048
CONVERSION_TIMEOUT_SECS=1800
DOCKER_SOCKET=/var/run/docker.sock
RUST_LOG=info,tensorrt_converter=debug
```

### TensorRT Docker Images Configuration

Stored in `config/images.toml`:
```toml
[[images]]
name = "TensorRT 8.6 - CUDA 12.0"
tag = "nvcr.io/nvidia/tensorrt:23.04-py3"
cuda_version = "12.0"
tensorrt_version = "8.6"

[[images]]
name = "TensorRT 10.3 - CUDA 12.6"
tag = "nvcr.io/nvidia/tensorrt:24.08-py3"
cuda_version = "12.6"
tensorrt_version = "10.3"
```

---

## Toolchain Configuration

### rust-toolchain.toml
```toml
[toolchain]
channel = "1.89.0"
components = ["rustfmt", "clippy"]
```

### Key Dependencies (Cargo.toml)

```toml
[package]
name = "tensorrt-converter"
edition = "2024"
rust-version = "1.89.0"

[dependencies]
dioxus = { version = "0.7", features = ["fullstack", "router"] }
tokio = { version = "1", features = ["full"] }
bollard = "0.18"                    # Docker API
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
uuid = { version = "1", features = ["v4", "serde"] }
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
toml = "0.8"
tempfile = "3"
tokio-stream = "0.1"

[dev-dependencies]
pretty_assertions = "1"
```

---

## Common Patterns

### Error Handling Pattern
```rust
// src/errors/app_error.rs
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Docker error: {0}")]
    Docker(#[from] bollard::errors::Error),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Conversion failed: {0}")]
    Conversion(String),
}
```

### Server Function Pattern
```rust
#[server]
async fn submit_conversion_job(
    model_name: String,
    format: ModelFormat,
    image_tag: String,
    gpu_id: GpuId,
    template_name: String,
) -> Result<JobId, ServerFnError> {
    // Validate inputs
    // Create job record in DB
    // Spawn conversion task
    // Return job ID
}
```

### Component Pattern
```rust
#[derive(Props, Clone, PartialEq)]
struct ProgressBarProps {
    job_id: JobId,
}

fn ProgressBar(props: ProgressBarProps) -> Element {
    let progress = use_resource(move || {
        let id = props.job_id.clone();
        async move { get_job_progress(id).await }
    });

    rsx! {
        // Clean RSX with no inline logic
    }
}
```

### Logging & Tracing (MANDATORY)

- **ALL log output MUST be structured JSON** — never use plain text formatting
- Use `tracing` crate for all instrumentation: `tracing::info!`, `tracing::error!`, `tracing::warn!`, `tracing::debug!`
- Initialize subscriber with JSON layer in `main.rs`:

```rust
use tracing_subscriber::{fmt, EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

tracing_subscriber::registry()
    .with(EnvFilter::from_default_env())
    .with(fmt::layer().with_target(true))
    .init();
```

- **Every `#[server]` function** must have `#[tracing::instrument(skip_all, fields(...))]` with relevant fields (job_id, model_name, etc.)
- **Every async task spawn** must carry a `tracing::Span` so logs from background jobs are correlated
- Use structured fields, not string interpolation: `tracing::info!(job_id = %id, status = "started", "conversion job started")` — NOT `tracing::info!("conversion job {} started", id)`
- **Error logs** must include the full error chain: `tracing::error!(error = ?err, "operation failed")`
- Log levels: `error` = unrecoverable failures, `warn` = recoverable issues, `info` = business events (job created/completed/failed), `debug` = internal state changes, `trace` = Docker API calls & raw output

---

## Things to AVOID

- **No `println!` / `eprintln!` / `dbg!`** — use structured `tracing` macros with JSON output exclusively
- **No `panic!` in production paths** — return `Result` instead
- **No hardcoded paths** — all paths come from config/env
- **No raw string matching for Docker image names** — use typed config
- **No manual JSON construction** — always derive `Serialize`/`Deserialize`
- **No synchronous file I/O** — use `tokio::fs` everywhere
- **No `clone()` spam** — prefer references, use `Arc` for shared ownership
- **No nested callbacks deeper than 2 levels** — extract to named functions
- **No magic numbers** — define as named constants or config values
- **No `#[allow(unused)]`** — remove dead code instead of silencing warnings
- **No CSS inline styles in RSX** — use TailwindCSS utility classes exclusively
- **No `git add` / `git commit` when `cargo clippy -- -D warnings` fails** — fix ALL warnings/errors first, then commit
- **No `git add .` or `git add -A`** — always stage specific files relevant to the change
- **No commits with broken builds** — every commit in history must compile cleanly with zero warnings
