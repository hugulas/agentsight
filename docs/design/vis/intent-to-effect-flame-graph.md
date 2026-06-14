# Intent-to-Effect Flame Graph

状态：已被新设计取代。

早期版本把 AgentSight 火焰图描述成：

```text
session -> semantic phase -> tool intent -> process tree -> effect
```

这个形态更像单次 run 的 trace tree。它保留因果展开，但没有严格定义
flame graph 需要的 collapsed stack aggregation，容易把“树形展示”和
“聚合火焰图”混在一起。

当前设计改为：

- 用本地小模型只生成 one-word semantic tags；
- 保留 AgentSight 精确的 `tool_call -> shell -> child process -> effect`
  provenance；
- 用 execution ownership / causal nesting 定义 folded stack；
- 分开生成 system footprint flamegraph 和 token/reasoning flamegraph；
- 用 behavior diff flamegraph 做跨 session / model / prompt 比较。

请优先阅读：

- [One-Word Semantic Tags And Task-Aligned Aggregation](one-word-semantic-tags.md)
- [One-Word Semantic Tags Research Plan](../papers/agentsight-task-grounded-system-effects-osdi-draft.md)

