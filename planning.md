# Workflow Execution Engine - Planning Document (Final Graph-Based Architecture)

## Overview
This document outlines the architecture for a high-performance, distributed, Graph-Based Workflow Execution Engine built in **Rust**.
The design prioritizes **keeping the Graph structure intact** at runtime for observability and simplicity, while leveraging **Rust's low-level optimizations (Arena Allocation, Zero-Cost Abstractions)** to ensure maximum performance.

## Core Requirements
1.  **Execution Model**: Graph Traversal. The runtime traverses the graph nodes directly.
2.  **Node Types**: `Start`, `End`, `Http`, `If/Else`, `Loop`, `Assign`, `Fork`, `Join`.
3.  **Concurrency**: Parallel execution of branches using a distributed Task Queue model.
4.  **Persistence**: Pause/Resume capable. State is fully serializable.
5.  **Performance**: High CPU utilization, minimal GC overhead.

## Architecture

### 1. The Compiler Layer (Translation & Optimization)
*   **Input**: JSON DSL (User-friendly, using String IDs).
*   **Process**:
    1.  **Parse**: Load JSON into Rust structs.
    2.  **Validate**: Check connectivity, variable usage.
    3.  **Index (Arena Mapping)**: Convert string-based IDs (`"node-A"`) into integer-based indices (`NodeIndex(usize)`).
    4.  **Flatten**: Store all nodes in a continuous memory block (`Vec<Node>`) for O(1) access.
*   **Output**: `Blueprint` (The optimized, read-only Graph).

### 2. The Runtime Data Model

#### A. `Blueprint` (The Map)
*   A static, immutable reference to the workflow logic.
*   Structure:
    ```rust
    struct Blueprint {
        nodes: Vec<Node>, // The Arena
        start_index: usize,
    }
    ```

#### B. `Instance` (The Traveler's Journal)
*   Represents a single run of a workflow.
*   **Serializable** to JSON for persistence.
*   Structure:
    ```rust
    struct Instance {
        id: Uuid,
        blueprint_id: String,
        // Shared State (Variables)
        context: DashMap<String, Value>,
        // Execution State
        status: InstanceStatus,
        // For Join Nodes: count how many branches arrived
        pending_joins: DashMap<NodeIndex, usize>, 
    }
    ```

#### C. `Task` (The Command)
*   A unit of work to be executed.
*   Structure:
    ```rust
    struct Task {
        instance_id: Uuid,
        node_index: usize, // Direct index into Blueprint.nodes
        flow_id: Uuid,     // Ancestry ID for tracking parallel branches
    }
    ```

### 3. Execution Flow (The Loop)

1.  **Engine Start**:
    *   User calls `run(blueprint_id)`.
    *   Engine creates `Instance`.
    *   Engine pushes initial `Task(StartNode)` to **Task Queue**.

2.  **Worker (Executor)**:
    *   Loops forever: `queue.pop()`.
    *   **Fetch**: Gets `Node` from `Blueprint` using `task.node_index`.
    *   **Execute**: Runs node logic (HTTP, Eval, etc.) against `Instance.context`.
    *   **Decide**:
        *   **Sequence**: Returns `Next(index)`. Worker pushes 1 new Task.
        *   **Fork**: Returns `Branches(vec![idx1, idx2])`. Worker pushes N new Tasks.
        *   **Join**:
            *   Decrements `Instance.pending_joins[node_index]`.
            *   If count == 0, pushes `Next` Task.
            *   Else, drops the task (wait for others).
        *   **Async**: Spawns a Tokio future (e.g., for HTTP). When done, that future pushes the Result Task back to Queue.

### 4. Project Structure

```
skript/
├── src/
│   ├── types.rs          # Core data structures (Value, Error)
│   ├── compiler/         # DSL -> Blueprint (Arena construction)
│   │   ├── mod.rs
│   │   └── parser.rs
│   ├── runtime/
│   │   ├── mod.rs
│   │   ├── engine.rs     # Orchestrator
│   │   ├── executor.rs   # Worker Logic
│   │   ├── state.rs      # Instance & Context definition
│   │   └── registry.rs   # Blueprint storage
│   └── nodes/            # Node Logic implementations
│       ├── mod.rs
│       ├── http.rs
│       ├── flow.rs       # If, Loop, Fork, Join
│       └── common.rs     # Start, End, Assign
└── examples/
    └── simple_flow.json
```

### 5. Implementation Phase 1 (Skeleton)
*   Define `Node` enum with `usize` pointers.
*   Implement `Compiler` to load JSON and build `Vec<Node>`.
*   Implement basic `Executor` loop processing a linear flow.