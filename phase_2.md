
# 1. Pause/Resume 实现，Runtime 需要统一支持 pause 和 resume 能力
- 根据 checkpoint 来检查是否被 pause,checkpoint就是每个节点被执行完或者一个函数被执行完，总之就是 workflow 级别的原子块执行完了，就要检查 pause 和 resume 标识
- 如果被 pause,则需要暂停当前 workflow 的执行，把状态全部持久化

# 2. 引入 Scheduler 来调度 workflow 执行，而且支持时间片划分策略，可以 round Robin 或者 priority based 或者 first come first serve
- 每个 workflow 执行的时候，需要根据时间片划分策略来决定执行哪个 workflow，如果时间片用完了，则需要暂停当前 workflow 的执行，把状态全部持久化

# 3. runtime 阶段的优化，对于那种可以连续的，连续执行的节点，可以合并为一个执行单元发布到 redis 或者线程任务，减少上下文切换的消耗，当然这里也要有个阈值，还要思考下整体架构，不能单独写这一个功能
- 比如参考 JIT 架构？这里需要讨论一下