# Skript 引擎设计文档 (Skript Engine Design Document)

## 1. 概述 (Overview)
Skript 是一个**高性能、图灵完备、支持并发**且**易于扩展**的工作流执行引擎。
它的设计哲学是 **"重编译，轻运行" (Heavy Compiler, Light Runtime)**：通过强大的编译器进行静态分析和优化，由一个精简、无 GC、全异步的 Rust 运行时（Runtime）执行。同时，它提供了一套**插件化**的节点系统，支持内置和自定义节点的无缝扩展。

核心目标：**榨干 CPU 性能，实现极致的吞吐量和低延迟，同时保持极佳的开发者体验 (DX)。**

## 2. 核心特性 (Core Features)
*   **高性能架构**: 
    *   **Rust 实现**: 无 GC，内存安全，零成本抽象。
    *   **Arena 内存布局**: 图节点存储在连续内存 (`Vec<Node>`)，通过索引 (`usize`) 访问，对 CPU 缓存极其友好。
    *   **Compiler 优化**: 支持节点融合 (Fusion)、预计算。
*   **并发与分布式**:
    *   **抽象运行时 (Runtime Abstraction)**: 支持 **In-Memory** (单机高性能) 和 **Redis** (分布式水平扩展) 两种后端。
    *   **Task 驱动**: 基于 `StateStore` 和 `TaskQueue` 的无状态 Worker 模型。
    *   **隐式并行 DSL**: 用户使用 `Parallel` 块，编译器自动生成底层的 `Fork` / `Join` 指令。
*   **插件化节点系统**:
    *   **FunctionHandler**: 统一的 Trait 定义，支持生命周期钩子 (`validate`, `execute`)。
    *   **编译期验证**: 自定义节点可以在 DSL 解析阶段拦截参数错误。
*   **图灵完备**:
    *   支持变量系统 (Context)、控制流 (If/Else)、循环 (Loop/ForEach)、函数调用。

## 3. 架构设计 (Architecture)

### 3.1 编译器层 (Compiler Layer - The Brain)
编译器负责将用户友好的 DSL 转换为机器高效的 Blueprint。

*   **输入**: DSL (YAML/JSON)。
*   **处理流程**:
    1.  **Parser (解析)**: 将 YAML/JSON 解析为 AST。
    2.  **Expander (展开)**: 将高层语法糖（如 `Parallel` 块）展开为底层的 `Fork/Join` 节点。
    3.  **Validator (验证)**: 
        *   结构检查 (连通性、环路)。
        *   **Function Validation**: 调用具体 `FunctionHandler` 的 `validate` 钩子检查节点参数。
    4.  **Optimizer (优化)**: 融合节点、预编译表达式。
    5.  **Codegen (生成)**: 构建 `Blueprint` (Arena Layout)。
*   **输出**: `Blueprint` (只读、静态、优化的指令图)。

### 3.2 运行时层 (Runtime Layer - The Muscle)
运行时是一个极简的虚拟机，负责执行 Blueprint。它已被抽象为基于接口的设计。

*   **Blueprint (静态图)**:
    ```rust
    struct Blueprint {
        nodes: Vec<Node>, // Arena: 所有节点及其指令
        start_index: usize,
    }
    ```
*   **Storage Layer (SPI)**:
    *   `TaskQueue`: 负责任务分发 (Push/Pop)。
        *   *In-Memory*: `tokio::sync::mpsc`
        *   *Redis*: `LPUSH` / `BRPOP`
    *   `StateStore`: 负责状态存储 (Variables, Join Counters, Metadata)。
        *   *In-Memory*: `DashMap`
        *   *Redis*: `Hash` (Variables) + Lua Scripts (Atomic Counters)
*   **Executor (执行器)**:
    *   **Worker**: 无状态计算单元。从 `TaskQueue` 抢占任务，执行 `Node` 逻辑，更新 `StateStore`，并将后续任务推回 `TaskQueue`。
    *   **Context**: 每个任务执行时的临时上下文，封装了对 `StateStore` 的异步访问。

### 3.3 插件接口 (Plugin Interface)

所有功能节点（包括内置的 Http, Log 和用户自定义节点）都必须实现此接口：

```rust
#[async_trait]
pub trait FunctionHandler: Send + Sync {
    fn name(&self) -> &str;
    fn validate(&self, params: &Value) -> Result<(), ValidationError>;
    async fn execute(&self, params: Value, ctx: &Context) -> Result<Value, ExecutionError>;
}
```

## 4. 高级调度与控制 (Phase 2 Design)

为了支持生产级的长运行工作流，引入调度器和暂停机制。

### 4.1 暂停/恢复 (Pause/Resume)
*   **Checkpoint 机制**: 
    *   每次节点执行完毕（原子操作结束）是检查点的天然时机。
    *   Worker 在生成后续任务前，检查 `InstanceMetadata` 中的状态。
*   **逻辑流程**:
    1.  用户发出 `Pause` 指令 -> 更新 `StateStore` 中 Instance 状态为 `Paused`。
    2.  Worker 完成当前 Node 执行。
    3.  Worker 检查状态：发现是 `Paused`。
    4.  **挂起 (Suspend)**: Worker **不**将后续任务推入 `TaskQueue` (Ready)，而是推入 `SuspendedPool` (持久化存储)。
    5.  用户发出 `Resume` 指令 -> 更新状态为 `Running` -> 将 `SuspendedPool` 中的任务移回 `TaskQueue`。

### 4.2 调度器 (Scheduler)
*   **目标**: 公平调度 (Round Robin) 和优先级控制 (Priority)。
*   **机制**:
    *   每个 Instance 拥有 `TimeSlice` (时间片) 或 `StepQuota` (步数配额)。
    *   Worker 执行节点消耗配额。
    *   当配额耗尽，Worker 强制执行 "Yield" 操作：将后续任务推入 `SuspendedPool` 而非 `ReadyQueue`。
    *   **Scheduler 组件**: 一个独立的后台进程/线程，负责轮询所有 Instance，按策略（如 RR）补充配额，并将任务从 `SuspendedPool` 激活到 `ReadyQueue`。

### 4.3 执行单元融合 (Execution Unit Fusion)
*   **目标**: 减少 Redis 交互和网络开销。
*   **逻辑**: 对于连续的、非阻塞的、无副作用（或副作用可控）的节点链（例如：`Assign -> Assign -> If -> Assign`），编译器或运行时可以将其视为一个 **Super Node**。
*   **JIT 策略**: 
    *   Worker 抢占到一个任务后，不仅执行当前 Node，如果后续 Node 是本地可执行且不需要重新调度的，则直接在本地继续执行，直到遇到 `Async I/O` (如 HTTP) 或 `Checkpoint` (时间片耗尽)。

## 5. 核心数据结构 (Rust Draft)

```rust
// 任务定义
struct Task {
    instance_id: Uuid,
    workflow_id: String,
    node_index: usize,
    // ...
}

// 实例元数据 (Phase 2)
struct InstanceMetadata {
    id: Uuid,
    status: WorkflowStatus, // Running, Paused, Terminated
    priority: u32,
    quota_remaining: u32, // Time Slice
}
```

## 6. 实施路线图 (Roadmap)

1.  **Phase 1: Core (已完成)**
    *   定义 `Blueprint`, `Instance`, `Task`。
    *   实现基础 Compiler (Parser -> Validator)。
    *   实现 `InMemory` 和 `Redis` 两种 Runtime 后端。
    *   实现分布式多进程 Worker 测试。
2.  **Phase 2: Control Plane (进行中)**
    *   **Metadata Management**: 在 `StateStore` 中引入实例元数据。
    *   **Pause/Resume**: 实现挂起池 (`SuspendedPool`) 和状态检查逻辑。
    *   **Scheduler**: 实现时间片轮转调度器。
3.  **Phase 3: Optimization (规划中)**
    *   **JIT / Node Fusion**: 优化连续节点的执行路径，减少队列交互。
    *   **Expression Engine Optimization**: 预编译表达式以提高求值速度。
