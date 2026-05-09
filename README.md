# TritonForge

TritonForge is a fullstack Rust web application for converting ONNX deep learning models into TensorRT engines. It provides a practical browser-based workflow for model upload, TensorRT configuration, GPU selection, job tracking, log inspection, and output management.

The project is designed for developers and machine learning engineers who want a repeatable way to build optimized TensorRT artifacts without managing every conversion manually from the command line.

## Why TritonForge

Building TensorRT engines often involves a sequence of manual steps: choosing the right TensorRT Docker image, selecting a GPU, preparing shape options, running `trtexec`, watching terminal output, collecting generated files, and keeping track of which model was built with which configuration.

TritonForge turns that workflow into an observable application flow. Each conversion is submitted as a job, executed inside a Docker container, tracked through its lifecycle, and stored with useful metadata for later inspection.

## Core Features

- Convert ONNX models into TensorRT engines.
- Upload ONNX files from the browser or select ONNX files from a server path.
- Select locally available or configured TensorRT Docker images.
- Detect and select NVIDIA GPUs through `nvidia-smi`.
- Configure TensorRT options such as dynamic shapes, workspace size, timing iterations, explicit batch, and FP16.
- Track conversion jobs with status, progress, timestamps, and container logs.
- Download completed model outputs.
- Organize completed models into deployment-oriented model groups.
- View completed models grouped by TensorRT image tag when building model groups.

## How It Works

At a high level, TritonForge follows this flow:

1. The user provides an ONNX model.
2. The user selects a TensorRT Docker image and target GPU.
3. TritonForge creates a conversion job.
4. The server stages the model and starts a Docker container.
5. The container runs TensorRT conversion with `trtexec`.
6. TritonForge records progress, logs, and job state.
7. The generated TensorRT output is stored for download or grouping.

The conversion lifecycle is intentionally explicit: pending, preparing, converting, finalizing, completed, or failed.

## Strengths

- **Focused ONNX workflow:** the application is built around ONNX-to-TensorRT conversion.
- **Docker-based isolation:** conversion jobs run inside TensorRT Docker containers instead of relying on host-installed TensorRT tooling.
- **GPU-aware execution:** users can choose the GPU used for each conversion job.
- **Observable jobs:** progress, logs, status, and metadata are available from the web UI.
- **Repeatable outputs:** completed jobs and model groups make it easier to compare builds and prepare deployment experiments.
- **Rust fullstack foundation:** Dioxus, Tokio, SQLx, SQLite, Docker/Bollard, and structured tracing provide a reliable async application stack.

## Requirements

- Rust `1.89.0` or newer compatible with the repo toolchain.
- Dioxus CLI (`dx`).
- SQLx CLI for database migrations.
- Docker daemon access.
- NVIDIA GPU driver and `nvidia-smi` for GPU detection.
- TensorRT Docker images, for example `nvcr.io/nvidia/tensorrt:*`.
- SQLite database configured through `DATABASE_URL`.
- Node.js/npm only if you need to update Tailwind-related frontend tooling.

## Environment Variables

Create a local `.env` file for development. The minimal required values are:

```bash
DATABASE_URL=sqlite://data/converter.db
DATA_DIR=/path/to/your/data
```

Supported runtime configuration:

| Variable | Purpose | Default |
| --- | --- | --- |
| `DATABASE_URL` | SQLite database URL used by SQLx. | `sqlite://data/converter.db` in local `.env` |
| `DATA_DIR` | **Required.** Parent directory for all runtime data. Subdirectories `uploads/`, `outputs/`, and `groups/` are created under this path. | _(none, must be set)_ |
| `MAX_UPLOAD_SIZE_MB` | Maximum upload size in MiB. | `2048` |
| `CONVERSION_TIMEOUT_SECS` | Maximum runtime for one conversion job. | `1800` |
| `DOCKER_SOCKET` | Docker daemon socket path. | `/var/run/docker.sock` |
| `TENSORRT_IMAGES_CONFIG` | Optional TOML file containing known TensorRT image entries. | `config/images.toml` |
| `RUST_LOG` | Log level filter for the application. | `info` |

## Running with Docker

The pre-built image is based on `nvcr.io/nvidia/cuda:11.7.0-base-ubuntu22.04`. It requires access to the host Docker socket and an NVIDIA-capable runtime so conversion containers can be spawned.

### Build the image

```bash
docker build -t tritonforge:latest .
```

### Run the container

```bash
docker run -d \
  --name tritonforge \
  --gpus all \
  -p 8080:8080 \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -v /your/data/path:/your/data/path \
  -e DATABASE_URL=sqlite:///your/data/path/converter.db \
  -e DATA_DIR=/your/data/path \
  -e RUST_LOG=info,tensorrt_converter=debug \
  tritonforge:latest
```

> **Important — `DATA_DIR` must use the same absolute path on the host and inside the container.**
>
> TritonForge runs inside Docker but spawns TensorRT conversion containers as sibling containers on the host Docker daemon. When it bind-mounts ONNX files into those sibling containers, Docker resolves the paths against the **host filesystem**, not the TritonForge container filesystem. If `DATA_DIR` differs between host and container, Docker will mount the wrong path (or fail with a not-found error) when launching conversion jobs.
>
> Always pass the same path on both sides of the `-v` flag:
> ```
> -v /your/data/path:/your/data/path   # host path == container path
> -e DATA_DIR=/your/data/path
> ```

---

## Development Setup

Install the Rust and Dioxus tooling:

```bash
cargo install dioxus-cli --locked
```

Start the fullstack development server:

```bash
dx serve --web --fullstack true
```

Generate Tailwind CSS after changing styles:

```bash
npx tailwindcss -i ./input.css -o ./assets/tailwind.css
```

During active frontend development, run Tailwind in watch mode in a separate terminal:

```bash
npx tailwindcss -i ./input.css -o ./assets/tailwind.css --watch
```

You can also bind the server explicitly:

```bash
dx serve --web --fullstack true --addr 127.0.0.1 --port 8080 --open false
```

## Development Commands

```bash
# Start the web app with hot reload
dx serve --web --fullstack true

# Generate Tailwind CSS once
npx tailwindcss -i ./input.css -o ./assets/tailwind.css

# Watch Tailwind input changes during UI development
npx tailwindcss -i ./input.css -o ./assets/tailwind.css --watch

# Format Rust code
cargo fmt

# Compile and lint with warnings treated as errors
cargo clippy -- -D warnings

# Run tests
cargo test
```

Recommended pre-commit check:

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
```

## Run In Release Mode

Run the fullstack app in release mode:

```bash
dx serve --web --fullstack true --release
```

With an explicit bind address and port:

```bash
dx serve --web --fullstack true --release --addr 127.0.0.1 --port 8080 --open false
```

## Build For Release

Create an optimized production build:

```bash
dx build --release
```

Create a deployment bundle:

```bash
dx bundle
```

## Testing

Run the full test suite:

```bash
cargo test
```

Run library tests only:

```bash
cargo test --lib
```

Run a specific integration test target:

```bash
cargo test --test docker_test
```

Docker and GPU-related checks depend on the local machine environment. A machine without Docker daemon access, NVIDIA drivers, or `nvidia-smi` may not exercise the full conversion path.

## Author

Dat Vo

- Contact: `vtdat58@gmail.com`
- GitHub: <https://github.com/dat58/TritonForge>

## License

This project is licensed under the MIT License.
