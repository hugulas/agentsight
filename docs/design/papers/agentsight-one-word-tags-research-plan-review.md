# OSDI Self Review: One-Word Semantic Tags Research Plan

被审文档：

`docs/design/papers/agentsight-task-grounded-system-effects-osdi-draft.md`

状态：第三版 research plan 自审记录。

## Review Iteration 1

Verdict: **borderline**

第一版把 one-word tags 方案写成了完整 claim/evaluation plan，但仍有几个顶会风险：

1. **像可视化/UI，不像系统贡献。**

   需要把贡献钉在形式化数据模型、不变量、metric conservation 和 provenance
   preservation 上，而不是“图更好看”。

2. **tag quality 的 oracle 不够硬。**

   需要明确 contract checker、human semantic adequacy、cluster purity、stability
   和 canonicalization collision，而不是只说“tag 有用”。

3. **baseline 可能不公平。**

   需要规定每个 baseline 回答同一组 analysis tasks，而不是比较不同形态的 UI。

4. **flamegraph claim 容易被攻击。**

   需要把 stack grammar 明确为 execution ownership / causal nesting，并用 injected
   loop/diff oracle 评估。

5. **缺少系统边界。**

   需要明确 small model 不可信，tag 不能改变 provenance，coverage/long-lived/
   concurrent cases 如何降级。

## Revision Applied

已修订：

- 新增 `Formal Data Model`，把 trajectory 写成 `(S, P, A, T, E, R, C)`。
- 新增 `Required Invariants`：tag contract、provenance preservation、effect
  conservation、metric conservation、authority separation、coverage honesty。
- 新增 `System-Under-Test Model`：components、durable state、trust boundaries、
  guarantees 和 assumptions。
- 新增 `Analysis Tasks`：Q1-Q7，要求 baseline 回答同一组可判定问题。
- 新增 C6：real-session behavior profiles。
- 加强 B1/B2/B3/B4 的 oracle 和 failure interpretation。
- 新增 `Industrial And Academic Value`，明确顶会贡献是 constrained semantic
  index + exact provenance aggregation，不是 prompt 命名本身。

## Review Iteration 2

Verdict: **weak accept as a research plan**

现在文档已经达到可以开始实现和实验的标准。它仍不是完整 OSDI paper，因为没有
结果，但作为 research plan 已具备：

- 明确 thesis；
- claim ledger；
- claim-to-experiment map；
- system-under-test model；
- named baselines；
- executable experiment blocks；
- robustness and scale experiments；
- run order；
- claim gates；
- negative-result handling；
- reproducibility path；
- implementation handoff。

## Remaining Risks

1. **One-word tags 可能太弱。**

   如果 semantic adequacy 或 cross-session stability 不够，必须回退到 per-repo
   constrained vocabulary 或 fixed taxonomy。

2. **User utility 可能不显著。**

   如果 B2 user/reviewer study 没有明显收益，应把论文收窄成 measurement/tooling
   paper，主打 behavior profiles 和 reproducibility。

3. **Flamegraph 单次 run 价值可能不够。**

   如果单次 run flamegraph 不强，应把主图转向 behavior diff flamegraph。

4. **底层 provenance 仍是根基。**

   如果 exact tool-to-effect provenance 在 detached/concurrent/late-attach cases
   下不稳定，plan 中的 coverage degradation 必须保守，否则会破坏论文可信度。

## Gate Before Implementation Expansion

先做最小实现和 R001-R003。只有当以下条件满足，才扩大到完整 experiment matrix：

- accepted tags satisfy the one-word contract after retry；
- folded stacks conserve source metrics；
- removing tags preserves exact provenance；
- transcript-only/system-only/fixed taxonomy baselines can answer the same Q1-Q7
  task format；
- at least one real session produces a useful prompt-tag footprint table and
  system/token folded stacks。
