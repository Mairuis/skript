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
*   **并发模型**:
    *   **Actor/Task 模式**: 基于 `Tokio` 异步运行时和 `Crossbeam` 无锁队列。
    *   **隐式并行 DSL**: 用户使用 `Parallel` 块，编译器自动生成底层的 `Fork` / `Join` 指令。
*   **插件化节点系统**:
    *   **ActionHandler**: 统一的 Trait 定义，支持生命周期钩子 (`validate`, `execute`)。
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
        *   **Action Validation**: 调用具体 `ActionHandler` 的 `validate` 钩子检查节点参数。
    4.  **Optimizer (优化)**: 融合节点、预编译表达式。
    5.  **Codegen (生成)**: 构建 `Blueprint` (Arena Layout)。
*   **输出**: `Blueprint` (只读、静态、优化的指令图)。

### 3.2 运行时层 (Runtime Layer - The Muscle)
运行时是一个极简的虚拟机，负责执行 Blueprint。

*   **Blueprint (静态图)**:
    ```rust
    struct Blueprint {
        nodes: Vec<Node>, // Arena: 所有节点及其指令
        start_index: usize,
    }
    ```
*   **Instance (动态状态)**:
    ```rust
    struct Instance {
        id: Uuid,
        // 变量存储: 支持并发读写的 DashMap
        context: DashMap<String, Value>, 
        // 执行指针: 记录所有活跃的 Thread (Token)
        tokens: DashMap<TokenId, Token>,
        // Join 计数器: 记录每个 Join 节点还差几个分支
        pending_joins: DashMap<NodeIndex, AtomicUsize>,
        status: InstanceStatus,
    }
    ```
*   **Executor (执行器)**:
    *   基于 `Tokio` 的 Worker 线程池。
    *   循环从全局 **Task Queue** (`crossbeam::channel`) 抢占任务。
    *   **Action Execution**: 调用 `ActionHandler::execute`，传入解析后的参数。

### 3.3 插件接口 (Plugin Interface)

所有功能节点（包括内置的 Http, Log 和用户自定义节点）都必须实现此接口：

```rust
#[async_trait]
pub trait ActionHandler: Send + Sync {
    /// 节点的唯一名称，对应 DSL 中的 `name` (e.g., "http_request")
    fn name(&self) -> &str;

    /// 编译期验证 (Compile Time)
    /// 检查 DSL 中的 params 是否合法 (必填校验、类型校验)
    fn validate(&self, params: &Value) -> Result<(), ValidationError>;

    /// 运行时执行 (Runtime)
    /// params: 已经由引擎解析了变量插值的参数
    /// ctx: 运行时上下文，可读取/写入变量
    async fn execute(&self, params: Value, ctx: &mut Context) -> Result<Value, ExecutionError>;
}
```

## 4. DSL 设计 (DSL Design)

DSL 旨在**人类可读**，屏蔽底层的复杂性。

1.  **Parallel Block (并行块)**:
    *   替代显式的 Fork/Join。
    *   DSL: `type: Parallel, branches: [nodes: [...], nodes: [...]]`
    *   Compiler: 自动插入 Fork 和 Join 指令。
2.  **Function/Action (通用节点)**:
    *   DSL: `type: Function, name: "my_action", params: {...}`
    *   支持通过 `${var}` 语法引用变量。
3.  **Variable Passing**:
    *   `params`: 输入参数映射。
    *   `output`: 指定结果写入哪个变量。
4.  **Control Flow**:
    *   `If`: 基于 Edge 或 Node 分支。
    *   `Iteration`: ForEach 循环。

## 5. 核心数据结构 (Rust Draft)

```rust
// 节点定义 (Runtime IR)
enum Node {
    // 融合节点：包含一系列纯计算指令
    Fused(Vec<Instruction>), 
    
    // 通用 Action 节点 (指向 Registry 中的 Handler)
    Action { 
        handler_name: String, 
        params: Value, // 预编译的参数模板
        output_var: Option<String> 
    },
    
    // 控制流节点 (底层指令，DSL 中可能是 Parallel 块)
    Fork(Vec<usize>), 
    Join { target: usize, expect: usize }, 
    
    // 迭代器
    Iteration(IterationConfig),
}
```

## 6. 实施路线图 (Roadmap)

1.  **Phase 1: Core (骨架)**
    *   定义 `Blueprint`, `Instance`, `Task`。
    *   定义 `ActionHandler` Trait。
    *   实现基础 Compiler (Parser -> Validator)。
2.  **Phase 2: Plugin System (插件)**
    *   实现 `ActionRegistry`。
    *   实现内置 Actions: `LogAction`, `AssignAction` (作为特殊 Action 或 Fused 指令), `SleepAction`。
    *   在 Compiler 中集成 `validate` 钩子。
3.  **Phase 3: Concurrency & Flow (并发与流)**
    *   实现 DSL 层的 `Parallel` 到 IR 层 `Fork/Join` 的转换逻辑 (Expander)。
    *   实现 Executor 的并发调度。
4.  **Phase 4: Advanced (高级)**
    *   `HttpAction` (带 Reqwest)。
    *   表达式引擎集成。