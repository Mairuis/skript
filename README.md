# Skript

> **A high-performance, distributed, and async-first workflow engine written in pure Rust.**

Skript is designed for building scalable orchestration systems where performance, concurrency, and type safety are non-negotiable. It adopts a **"Heavy Compiler, Light Runtime"** philosophy: moving as much work as possible (validation, optimization, graph flattening) to the compilation phase, leaving the runtime to be a thin, ultra-fast execution layer.

## 🌟 Why Skript?

### 🚀 Unmatched Performance
*   **Zero GC:** Built on Rust, ensuring predictable latency and memory usage.
*   **Arena Memory Layout:** Graph nodes are stored in contiguous memory (`Vec<Node>`) for maximum CPU cache locality.
*   **Async Runtime:** Powered by `tokio`, Skript handles thousands of concurrent workflows with a minimal thread footprint.

### 🌐 Truly Distributed & Scalable
*   **Pluggable Storage:** Switch between **In-Memory** (for single-node speed) and **Redis** (for multi-node scaling) with zero code changes.
*   **Stateless Workers:** Spin up any number of worker instances on different machines. They coordinate via the centralized task queue and state store.
*   **Atomic Joins:** Uses Lua scripts for atomic `Fork`/`Join` operations across distributed nodes.

### 🧠 Intelligent Compiler
*   **Human-Readable DSL:** Design workflows with a YAML-based DSL that is concise, intuitive, and easy to understand for both developers and business users.
*   **Optimizing Compiler:** Performs node fusion, dead code elimination, and expression pre-computation before the workflow ever runs.
*   **Safety First:** Validates node parameters and structural integrity at compile-time, catching errors early.
*   **Rich DSL:** Supports `Parallel`, `If/Else`, `Loop`, and custom function nodes out of the box.

---

## 🏗 Architecture

The system is divided into two distinct layers:

![Skript Architecture Diagram](overview.svg)

1.  **Compiler ("The Brain"):** Transforms high-level constructs (like `Parallel` blocks) into primitive low-level instructions (`Fork`, `Join`, `Jump`). It produces a static, read-only `Blueprint`.
2.  **Runtime ("The Muscle"):** A virtual machine that executes `Blueprints`. It manages the `TaskQueue` and `StateStore`, ensuring fault-tolerant execution.

---

## ⚡ Quick Start

### 1. Define a Workflow
Create a file named `flow.yaml`:

```yaml
name: "data-processing-pipeline"
nodes:
  - id: "start"
    kind: "Parallel"
    branches:
      - - id: "process_a"
          kind: "Action"
          action: "http_post"
          params: 
            url: "https://api.example.com/v1/data"
      
      - - id: "process_b"
          kind: "Action"
          action: "log"
          params:
            msg: "Processing parallel stream B..."

  - id: "aggregate"
    kind: "Action"
    action: "log"
    params:
      msg: "All parallel tasks completed. Aggregating results."
```

### 2. Run it Locally
```bash
cargo run -- run flow.yaml
```

### 3. Run the Stress Test
Skript includes an auto-tuning benchmark tool that pushes your CPU to the limit.
```bash
# Spawns 2000 concurrent tasks per workflow to measure scheduler overhead
cargo run --release -- bench
```

**Recent Benchmark Results (M1 Max, 10 Cores):**
*   **Throughput:** ~19,500 tasks/sec
*   **Latency:** 0.1ms scheduling overhead
*   **Concurrency:** Successfully saturated all cores with minimal lock contention.

---

## 🔮 Future Outlook

Skript is evolving into a general-purpose orchestration platform. Our roadmap focuses on ecosystem and observability:

1.  **🔭 Observability & Tracing**
    *   Native OpenTelemetry (OTEL) integration for distributed tracing across workers.
    *   Real-time metrics dashboard (Prometheus exporter).

2.  **📦 Ecosystem Expansion**
    *   **WASM Compilation:** Run the Skript compiler in the browser to build a web-based workflow editor.
    *   **Language Bindings:** Python and Node.js SDKs to define workflows programmatically.

3.  **💾 Advanced Persistence**
    *   **Postgres/SQL Backend:** For long-running workflows requiring ACID transactions and historical auditing.
    *   **Journaling:** Event-sourced persistence for "time-travel" debugging.

4.  **⚡ JIT Compilation**
    *   Explore compiling frequently used Blueprint patterns directly into native code using `cranelift` for even greater throughput.