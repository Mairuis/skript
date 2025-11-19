# Skript 系统实现文档 (Skript System Implementation Document)

本文档基于当前代码库 (`src/`, `tests/`) 的实际分析生成，详细描述了 Skript 引擎的实现架构、核心逻辑以及与设计文档的对比。

## 1. 系统架构 (System Architecture)

Skript 遵循 **"Heavy Compiler, Light Runtime"** 的设计理念，将工作流的执行分为两个明确的阶段：编译期和运行期。

```mermaid
graph TD
    DSL[YAML/JSON Workflow] -->|Parse| AST[DSL Models]
    AST -->|Expand| AST_Expanded[Expanded DSL]
    AST_Expanded -->|Compiler| BP[Blueprint (JSON)]
    
    subgraph Compiler Layer
        Expander -->|Desugar| Parallel_to_ForkJoin
        Transformer -->|Index & Map| Blueprint_Gen
    end
    
    BP -->|Load| Engine
    
    subgraph Runtime Layer
        Engine -->|Prepare| ExecCache[Executable Cache (Vec<Box<Node>>)]
        ExecCache -->|Spawn| Task[Task Queue]
        Worker -->|Poll| Task
        Worker -->|Execute| Node_Impl
        Node_Impl -->|Syscall| Engine
        Node_Impl -->|Function| Handler[FunctionHandler]
    end
```

## 2. 编译器层 (Compiler Layer)

代码位置: `src/compiler/`

编译器负责将用户友好的 DSL 转换为运行时可高效执行的 `Blueprint`。

### 2.1 流程
1.  **Expansion (展开)**: 
    *   由 `src/compiler/expander.rs` 处理。
    *   核心逻辑是将高阶节点（如 `Parallel`）转换为底层的图元节点（`Fork`, `Join`）。
    *   这是一个递归过程，支持嵌套结构。
2.  **Indexing (索引)**:
    *   建立 `NodeId (String) -> NodeIndex (usize)` 的映射。
    *   检查重复 ID。
3.  **Transformation (转换)**:
    *   将 DSL 的 `Node` 转换为 `BlueprintNode`。
    *   解析边的连接关系，将 `target_id` 转换为 `target_index`。
    *   将参数 (`params`) 序列化为 JSON `Value`，并注入系统参数（如 `next`, `output`, `branches` 中的 `target`）。

### 2.2 数据结构
*   **Input**: `crate::dsl::Workflow` (由 `serde_yaml` 解析)
*   **Output**: `crate::runtime::blueprint::Blueprint` (包含 `Vec<BlueprintNode>` 和 `start_index`)

## 3. 运行时层 (Runtime Layer)

代码位置: `src/runtime/`

运行时采用基于 Actor/Token 的异步执行模型。

### 3.1 Engine (`engine.rs`)
*   **Blueprints**: 存储原始的 Blueprint 定义。
*   **Executable Cache**: 存储 JIT 实例化后的节点列表 (`Arc<Vec<Box<dyn Node>>>`)。
    *   当 Blueprint 首次被调用时，Engine 会遍历其中的 `BlueprintNode`，查找 `NodeRegistry`，并调用 `NodeDefinition::prepare` 实例化具体的 `Node` 对象。
*   **Task Loop**:
    *   拥有一个 `mpsc::Sender<Task>` 和 `Receiver`。
    *   **Worker**: 单个循环不断从 Channel 拉取 `Task` 并执行。
    *   **Syscall**: 提供 `jump`, `fork`, `wait` 接口供节点调用，用于产生新的 Task。

### 3.2 Context (`context.rs`)
*   **Variables**: 使用 `DashMap<String, Value>` 存储实例变量，支持并发读写。
*   **Pending Joins**: 使用 `DashMap<NodeIndex, AtomicUsize>` 记录 Join 节点的剩余等待次数。

### 3.3 Node Traits (`node.rs`)
*   **`NodeDefinition`**: 负责节点的元数据和工厂方法 (`prepare`)。
    *   `prepare`: 在 Blueprint 加载时调用，用于预编译表达式、解析静态参数。
*   **`Node`**: 负责运行时的执行逻辑。
    *   `execute`: 接收 `Context`, `Task`, `Syscall`，返回 `Future`。

## 4. 节点系统实现 (Node System)

### 4.1 核心节点 (`src/nodes/flow.rs`, `common.rs`)
*   **Start / End**: 简单的流程开始和结束标记。
*   **Fork**: 调用 `syscall.fork(targets)`，向任务队列发送多个并发 Task。
*   **Join**: 
    *   利用 `Context` 中的 `AtomicUsize` 计数器。
    *   每次执行原子减一。如果减至 1 (变为 0 之前的最后一个状态)，则继续执行；否则调用 `syscall.wait()` (当前实现为空，即终止当前 Task)。
*   **If**:
    *   使用 `evalexpr` 库。
    *   在 `prepare` 阶段预编译表达式 AST。
    *   在 `execute` 阶段将 Context 变量注入求值环境。

### 4.2 Function 节点 (`src/nodes/function.rs`)
*   **FunctionNode**: 这是一个通用包装器。
    *   **变量插值**: 在执行前扫描 `params`，将 `${var}` 替换为 `Context` 中的实际值。
    *   **委托**: 调用内部 `Arc<dyn FunctionHandler>::execute`。
    *   **输出**: 将结果写入 `output` 指定的变量。

### 4.3 内置 Functions
*   **HttpFunction (`src/actions/http.rs`)**:
    *   基于 `reqwest`。
    *   支持动态 URL, Method, Headers, Body。
    *   验证逻辑 (`validate`): 检查 `url` 参数是否存在。

## 5. 与设计文档的对比 (Comparison vs Design)

| 特性 | 设计文档 (Design) | 实际实现 (Implementation) | 状态 |
| :--- | :--- | :--- | :--- |
| **DSL 结构** | Parallel Block, Functions | 完全实现，Expander 逻辑正确 | ✅ 一致 |
| **内存布局** | Arena (`Vec<Node>`) | 实现为 `Vec<Box<dyn Node>>` (Trait Object) | ✅ 一致 |
| **验证时机** | **Compiler Phase** (编译期) | **Loader Phase** (运行时加载 Blueprint 时) | ⚠️ **偏差** |
| **表达式** | 计划在 "Phase 4" | 已通过 `evalexpr` 实现，并在 `IfNode` 中使用 | 🚀 超前 |
| **Join 逻辑** | `expect` 计数器 | 基于 `AtomicUsize` 的无锁实现 | ✅ 一致 |
| **插件系统** | `FunctionHandler` Trait | 已实现，通过 `NodeRegistry` 注册 | ✅ 一致 |

### 关键偏差说明
**验证时机**: 设计文档希望在 `Compiler::compile` 阶段就调用 `FunctionHandler::validate` 抛出错误。目前的实现中，`Compiler` 仅做结构转换。`validate` 方法存在于 `NodeDefinition` trait 中，但目前似乎仅在单元测试或手动调用中生效，Engine 的 `prepare` 阶段主要调用 `prepare` 方法（虽然 `prepare` 内部可能会做检查，但主要用于实例化）。

## 6. 总结 (Summary)
Skript 目前已经完成了一个功能完备的核心引擎。它成功实现了设计文档中关于 **高并发 (Fork/Join)**、**插件化 (FunctionHandler)** 和 **静态图编译 (Blueprint)** 的核心构想。虽然在验证逻辑的执行时机上与设计稍有出入，但这不影响运行时的正确性和性能。

代码结构清晰，模块化程度高，易于进行后续的扩展（如添加更多内置节点、优化调度器等）。
