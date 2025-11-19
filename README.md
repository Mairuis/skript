# Skript Engine

**Skript** is a high-performance, Turing-complete, concurrent workflow execution engine written in Rust.
Its design philosophy is **"Heavy Compiler, Light Runtime"**: leveraging a powerful compiler for static analysis and optimization, executed by a streamlined, GC-free, fully asynchronous runtime.

> **Performance Goal**: Squeeze every bit of CPU performance, achieving extreme throughput and low latency while maintaining excellent Developer Experience (DX).

## 🚀 Key Features

*   **High-Performance Architecture**:
    *   **Pure Rust**: Zero GC, memory safety, and zero-cost abstractions.
    *   **Arena Memory Layout**: Graph nodes stored in contiguous memory (`Vec<Node>`), accessed via index (`usize`), ensuring cache friendliness.
    *   **Compiler Optimization**: Supports node fusion, pre-computation, and flattening.
*   **True Parallelism**:
    *   **Async Runtime**: Built on `tokio`, supporting true multi-threaded execution via a work-stealing worker pool.
    *   **Implicit Parallelism DSL**: Users use `Parallel` blocks; the compiler automatically generates low-level `Fork`/`Join` instructions.
*   **Pluggable System**:
    *   **FunctionHandler Trait**: Unified interface for extending functionality with lifecycle hooks (`validate`, `execute`).
    *   **Compile-Time Validation**: Custom nodes can intercept parameter errors during the DSL parsing phase.

## 📊 Benchmark Results

We tested the engine's scheduling overhead and parallel execution capability using a stress test workflow.

**Scenario**: A `Parallel` node spawning **2000 concurrent branches**, each calculating `Fibonacci(20)` (CPU intensive).

| Metric | Value |
| :--- | :--- |
| **Total Concurrent Tasks** | 2,000 |
| **Worker Threads** | 20 (on 10-core CPU) |
| **Execution Time** | **0.1025s** |
| **Throughput** | **~19,517 tasks/sec** |

*Note: This benchmark measures internal scheduling and execution overhead. The engine successfully saturated all available cores with minimal locking contention.*

## 🏗 Architecture

### 1. Compiler Layer ("The Brain")
Responsible for transforming user-friendly DSL into machine-efficient Blueprints.

*   **Input**: YAML/JSON DSL.
*   **Process**:
    1.  **Parser**: YAML -> AST.
    2.  **Expander**: Desugars high-level constructs (like `Parallel`) into primitive `Fork/Join` nodes.
    3.  **Validator**: Checks structural integrity and invokes `FunctionHandler::validate`.
    4.  **Optimizer**: Node fusion and expression pre-compilation.
    5.  **Codegen**: Generates the `Blueprint` (flat array layout).
*   **Output**: `Blueprint` (Read-only, static, optimized instruction graph).

### 2. Runtime Layer ("The Muscle")
A minimalist virtual machine that executes the Blueprint.

*   **Blueprint**: `Vec<Node>` (Arena).
*   **Storage Layer (SPI)**:
    *   **TaskQueue**: Distributes tasks (In-Memory `mpsc` or Redis `List`).
    *   **StateStore**: Manages variables and counters (In-Memory `DashMap` or Redis `Hash`).
*   **Executor**:
    *   **Workers**: Stateless computation units. They pop `Task`s, execute `Node` logic, update `StateStore`, and push new `Task`s.
    *   **Context**: Ephemeral access to state for each execution.

## 🛠 Usage

### CLI
To run a workflow file:

```bash
cargo run -- run ./dsl_examples/complex_flow.yaml
```

### Running Tests
To verify parallel execution performance:

```bash
cargo test --test parallel_execution_test --release -- --nocapture
```

### Automated Stress Test & Auto-Tuning
Skript includes a built-in benchmarking tool that automatically detects your CPU core count, adjusts worker threads, and ramps up load until it finds the peak throughput of your machine.

To run the automated stress test:

```bash
cargo run --release -- bench
```

*This will start an adaptive benchmark that pushes the engine to its limits (up to 100k concurrent tasks) using CPU-bound workloads.*

## 🔌 Plugin Interface

All functional nodes (including built-in Http, Log, and custom nodes) implement this trait:

```rust
#[async_trait]
pub trait FunctionHandler: Send + Sync {
    fn name(&self) -> &str;
    fn validate(&self, params: &Value) -> Result<(), ValidationError>;
    async fn execute(&self, params: Value, ctx: &Context) -> Result<Value, ExecutionError>;
}
```

## 🗺 Roadmap

1.  **Phase 1: Core (Completed)** ✅
    *   Blueprint, Instance, Task definitions.
    *   Compiler (Parser -> Validator).
    *   In-Memory & Redis Runtime Backends.
    *   **True Parallel Execution**.
2.  **Phase 2: Control Plane (In Progress)** 🚧
    *   Metadata Management.
    *   Pause/Resume mechanisms (SuspendedPool).
    *   Time-slice Scheduler.
3.  **Phase 3: Optimization**
    *   JIT / Node Fusion.
    *   Expression Engine pre-compilation.
