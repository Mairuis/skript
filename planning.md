# 工作流执行引擎 - 规划文档 (Planning Document)

## 概述
目标是设计并实现一个图灵完备的工作流执行引擎，它能够解释和执行通过领域特定语言（DSL，例如 JSON/YAML）定义的工作流。该引擎将支持各种节点类型、动态变量管理、状态持久化、错误处理，以及至关重要的并行执行能力。通过引入编译器进行优化，并采用分布式运行时模型，以实现可伸缩性和可观察性。

## 核心需求 (来自用户)
1.  **节点类型**: 支持 `Start`（开始）、`End`（结束）、`HTTP Request`（HTTP 请求）、`If/Else`（条件判断）、`Loop`（循环）、`Assign`（变量赋值）、`Function Call`（函数调用/子工作流）。
2.  **图灵完备性**: 确保健壮的变量系统（读写、作用域）、条件判断、循环结构和函数/子工作流调用，满足图灵完备性。
3.  **执行模型**: 解释执行，明确支持状态持久化和恢复（暂停/继续）。
4.  **数据流**: 定义清晰的节点间数据传递机制和执行上下文管理。
5.  **错误处理**: 实现节点执行失败时的策略，包括回滚和重试。
6.  **API 设计**: 提供清晰的 API 用于创建、执行和监控工作流。
7.  **可扩展性**: 易于添加新的节点类型，无需修改核心引擎逻辑。
8.  **序列化**: 支持 JSON/YAML 格式的工作流定义。
9.  **并行执行**: 支持多线程/分布式并行执行，处理分支流程（例如，一个 `Start` 节点引出多条并行路径）。
10. **编译阶段**: 引入编译器，处理 DSL，进行验证和优化（例如，合并顺序节点），生成高效的中间表示（IR），供运行时使用。
11. **分布式运行时**: 运行时应支持多个“执行器（Executors）”拉取任务，允许节点状态在内存中管理或存储于共享状态存储（如 Redis），从而支持暂停、恢复和步进调试（step-debugging）功能。

## 提议的架构

架构将分为三个主要层级：**定义层（Definition）**、**编译层（Compilation）** 和 **运行时层（Runtime）**。

### 1. 定义层 (Definition Layer - DSL)
*   **目的**: 用户友好、人类可读（且可编辑）的工作流定义。
*   **格式**: JSON（初步，未来可兼容 YAML）。
*   **结构**: 节点及其属性的列表，可能包含显式边或隐式的 `next` 指针。

### 2. 编译层 (Compilation Layer - 编译器)
*   **输入**: 原始工作流定义（DSL）。
*   **输出**: 一个经过优化、高效的 `CompiledBlueprint`（中间表示 - IR），供运行时核心高效执行。
*   **关键职责**:
    *   **验证**:
        *   根据预定义的节点类型和属性进行模式验证。
        *   图结构验证（例如，检测不可达节点、潜在死锁、确保所有路径都通向 `End` 节点或明确的错误）。
        *   变量使用验证（如果可能，检查未定义的变量引用）。
    *   **优化阶段 (Optimization Passes)**:
        *   **节点合并 (Node Fusion / Basic Block Optimization)**: 将连续的、同步的、非分支节点序列（例如，`Assign` -> `Assign` -> `Log`）合并为一个 `CompositeNode` 或 `SuperNode`。这减少了任务入队和上下文切换的次数，显著提高了简单顺序操作的性能。
        *   **图展平/索引**: 将节点列表转换为更高效的图表示（例如，邻接列表/映射），以便在执行期间快速查找。
        *   **静态变量分析**: 识别在整个工作流中保持不变的变量或可以预先计算的变量。
*   **输出结构 (Compiled Blueprint)**:
    *   节点 ID 到编译后节点对象的映射。
    *   用于导航的显式邻接列表/映射。
    *   工作流的元数据。

### 3. 运行时层 (Runtime Layer - 分布式执行模型)

该层实现“令牌驱动（Token-Driven）”和“基于队列（Queue-Based）”的执行模型，以支持并行性、持久性和分布式执行。

#### A. 核心组件

1.  **执行实例 (Execution Instance - 工作流运行状态)**:
    *   代表工作流的一个独立运行实例。
    *   **`WorkflowContext`**: 管理所有工作流作用域的变量及其值。这包括一个支持 `Loop` 和 `Function Call` 节点嵌套作用域的强大**变量系统**。它必须是可序列化的。
    *   **`ActiveTokens`**: 当前所有活跃执行令牌的集合。每个令牌代表一个独立的执行路径。
    *   **`ExecutionHistory`**: 执行过的节点、错误和状态变更的日志（用于监控和调试）。
    *   **`Status`**:（例如，`RUNNING`（运行中）、`PAUSED`（已暂停）、`COMPLETED`（已完成）、`FAILED`（失败））。
    *   **`CurrentNodeId` / `ProgramCounter`**: 当前被活跃令牌处理的节点 ID。

2.  **执行令牌 (Execution Token)**:
    *   **`id`**: 令牌的唯一标识符。
    *   **`currentNodeId`**: 该令牌当前指向的节点 ID。
    *   **`scopeId`**: 其局部变量作用域的标识符（用于嵌套调用/循环）。
    *   **`parentTokenId`**: 用于追踪分支/合并的血缘关系。
    *   **`status`**:（例如，`PENDING_EXECUTION`（待执行）、`WAITING_FOR_JOIN`（等待合并）、`COMPLETED`（已完成）、`ERROR`（错误））。

3.  **状态存储 (State Store - 持久化存储)**:
    *   **目的**: 将工作流的运行时状态外部化，以实现持久性、恢复和分布式执行。
    *   **实现**: 抽象接口，初步实现 `InMemoryStateStore`（内存状态存储），后续可切换为 `RedisStateStore`。
    *   **存储内容**:
        *   `ExecutionInstance` 对象（包括 `WorkflowContext` 和 `ActiveTokens`）。
        *   可能还有节点特定的临时数据。

4.  **任务队列 (Task Queue)**:
    *   **目的**: 解耦节点执行与调度器，允许多个执行器并行处理。
    *   **实现**: 抽象接口，初步实现 `InMemoryTaskQueue`（内存任务队列），后续可切换为 `RedisTaskQueue`。
    *   **任务结构**: 任务通常包含：`workflowInstanceId`、`tokenId`、`nodeIdToExecute`。
    *   **机制**: 当一个节点完成时，它确定下一个节点（或多个节点），并将相应的任务推送到此队列。

#### B. 带有并行和分布式模型的执行流程

1.  **初始化**:
    *   用户调用 `workflowEngine.start(workflowDefinition)`。
    *   `Compiler` 接收 `workflowDefinition`（DSL）并生成 `compiledBlueprint`（IR）。
    *   在 `StateStore` 中创建一个 `ExecutionInstance`。
    *   识别 `Start` 节点。为 `Start` 节点创建一个初始 `ExecutionToken`，并与 `ExecutionInstance` 关联。
    *   将一个任务（`workflowInstanceId`、`tokenId`、`startNodeId`）推送到 `TaskQueue`。

2.  **执行器循环 (Executor Loop)**:
    *   多个 `Executors`（可以是独立的进程/线程/容器）持续从 `TaskQueue` 拉取新任务。
    *   当 `Executor` 拉取一个任务时：
        *   它从 `StateStore` 中检索 `ExecutionInstance` 和相关的 `ExecutionToken`。
        *   它识别 `nodeIdToExecute` 并调用相应的 `NodeHandler`。
        *   `NodeHandler` 执行节点的逻辑（例如，发出 HTTP 请求、赋值变量）。
        *   **并发处理 (Fork)**: 如果一个节点（例如，`Parallel` 节点或具有多个标记为并行执行的出线的 `Start` 节点）导致多个后续节点，`NodeHandler` 将：
            *   将当前令牌标记为 `COMPLETED` 或 `FORKED`。
            *   为每条并行路径创建新的 `ExecutionToken`。
            *   将每个新令牌对应的任务推送到 `TaskQueue`。
        *   **并发处理 (Join)**: 如果一个节点是 `Join` 节点，其 `NodeHandler` 将：
            *   检查 `StateStore` 中的 `ExecutionInstance`，以查看所有预期的（来自相应分支的）入站令牌是否已到达。
            *   如果未全部到达，当前令牌将进入 `WAITING_FOR_JOIN` 状态（暂不推送新任务）。
            *   如果所有令牌都已到达，它将把已合并的令牌标记为 `COMPLETED`，并为下一步创建一个新的令牌，然后推送相应的任务。
        *   **状态更新**: 执行完成后，`Executor` 在 `StateStore` 中更新 `ExecutionInstance`（新的变量值、令牌状态、历史记录）。

3.  **持久化与恢复**:
    *   由于所有关键状态都存储在 `StateStore` 中，任务存储在 `TaskQueue` 中，系统本质上是持久化的。
    *   **暂停**: 停止所有执行器从 `TaskQueue` 拉取任务。工作流状态保留在 `StateStore` 中。
    *   **恢复**: 重新启动执行器。它们将从上次停止的地方继续拉取任务。
    *   **步进调试**: 可以提供一个 API，针对特定的 `ExecutionInstance`，从 `TaskQueue` 中逐一 `pop` 任务并执行，同时观察 `StateStore` 中的状态变化。

4.  **错误处理**:
    *   **节点级重试**: `NodeHandler` 可以在失败前实现重试逻辑。
    *   **工作流级错误**: 如果一个节点在重试后仍然失败，`ExecutionToken` 可以被标记为 `ERROR`。
    *   **恢复**: `StateStore` 可以存储错误信息。一个独立的机制（例如，“错误处理”节点或外部监控器）可以检测 `ERROR` 令牌并触发恢复操作（例如，回滚、从上一个成功点重新启动、发送通知）。

#### C. API 设计 (高层)
*   `workflowEngine.define(workflowDefinition: object)`: 注册一个工作流（编译后）。
*   `workflowEngine.start(workflowId: string, initialContext?: object)`: 启动一个新的工作流实例（如果未编译则先编译），返回 `workflowInstanceId`。
*   `workflowEngine.resume(workflowInstanceId: string)`: 恢复一个已暂停的工作流。
*   `workflowEngine.pause(workflowInstanceId: string)`: 暂停一个正在运行的工作流。
*   `workflowEngine.step(workflowInstanceId: string)`: 为调试目的，执行一个已暂停工作流的单个步骤。
*   `workflowEngine.monitor(workflowInstanceId: string)`: 获取当前状态、活跃令牌和上下文。
*   `workflowEngine.addNodeType(type: string, handler: NodeHandler)`: 用于扩展新的节点类型。

## 初步实现步骤

1.  **项目设置**: 初始化 Node.js 项目，配置 TypeScript，安装 `axios`（用于 HTTP 节点）、`uuid`（用于唯一 ID）、`lodash`（用于工具函数）。
2.  **目录结构**:
    *   `src/types/`: 所有接口定义，包括 DSL、编译后的 IR、运行时状态、令牌。
    *   `src/compiler/`: 负责解析、验证和优化工作流的逻辑。
    *   `src/runtime/core/`: `StateStore`（接口及 `InMemoryStateStore` 实现）、`TaskQueue`（接口及 `InMemoryTaskQueue` 实现）、`ExecutionEngine`（协调器）。
    *   `src/runtime/executors/`: `Executor` 类。
    *   `src/runtime/nodes/`: 每个 `NodeHandler` 的实现（Start、End、Assign、If、Loop、Parallel、HTTP、FunctionCall）。
    *   `src/examples/`: 示例工作流定义（DSL）。
    *   `src/index.ts`: API 的主入口点。
3.  **定义核心接口**: `IWorkflowDefinition`、`ICompiledNode`、`IExecutionToken`、`IWorkflowContext`、`IStateStore`、`ITaskQueue`、`INodeHandler`。
4.  **实现 `InMemoryStateStore` 和 `InMemoryTaskQueue`**: 这些将作为分布式组件的初步内存模拟实现。
5.  **构建一个基本编译器**: 一个直通版本，负责将 DSL 转换为 `CompiledBlueprint`，初步侧重于图索引而非深度优化。
6.  **实现 `ExecutionEngine`**: 用于管理 `ExecutionInstances` 并与 `StateStore` 和 `TaskQueue` 交互。
7.  **实现 `Executor`**: 主要的工作循环。
8.  **实现基本节点处理器**: `StartNode`、`EndNode`、`AssignNode`、`LogNode`（用于简单的输出和测试）。
9.  **实现并行/分支节点 (Parallel/Fork Node)**: 这对于测试并发执行模型至关重要。
