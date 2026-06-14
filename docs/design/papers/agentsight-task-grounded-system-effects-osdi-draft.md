# AgentSight: 用单词语义标签聚合 AI Agent 的系统行为

最后更新：2026-06-13

阶段：research-plan

来源：`docs/design/vis/one-word-semantic-tags.md`，覆盖旧的
task-verdict / claim-checker research plan。

## Thesis

AgentSight can make AI agent runs easier to understand, compare, and debug by
combining exact tool-to-system provenance with local small-model one-word tags.
The key mechanism is a constrained semantic layer that names sessions, prompts,
subagents, and LLM calls with single-word tags, then uses exact provenance to
aggregate system and token footprints by those tags. This is stronger than
transcript-only tracing, system-only observability, fixed taxonomies, or
free-form LLM summaries because it preserves system facts while adding a cheap,
reproducible semantic grouping key.

中文表述：

> AgentSight 的研究贡献不是自动判断 agent 是否完成任务，而是把 agent
> transcript 中的语义结构压缩成可聚合的单词标签，并把这些标签接到精确的
> tool/process/file/network provenance 上，从而支持 prompt-level footprint、
> subagent cost/effect 分析、task-aligned flamegraph 和跨 session 行为差异分析。

## Paper Type

- Type: new system + measurement/tooling.
- Target venue: OSDI / SOSP / EuroSys.
- Artifact status: existing AgentSight prototype plus new query-time semantic
  tagging and aggregation layer.
- Main reviewer risk: the work may look like a visualization/UI feature unless
  we demonstrate a falsifiable data model, strong baselines, rigorous tag
  quality metrics, aggregation correctness, and user-facing analysis benefits.

## 1. Problem

AI coding agents now produce long sessions with user prompts, assistant turns,
tool calls, subagents, shell commands, child processes, file writes, network
connections, and token-heavy reasoning loops. Existing tools split the picture:

- LLM observability tools show prompt/tool/model spans, but not actual OS
  footprint.
- System observability tools show process/file/network events, but not which
  prompt or subagent phase they belong to.
- Free-form LLM summaries can describe a run, but they are expensive,
  nondeterministic, difficult to reproduce, and easy to over-trust.
- Fixed task taxonomies are stable, but too rigid for arbitrary coding-agent
  work across repositories.

AgentSight already has the important systems substrate: captured system effects
can be traced through `tool_call -> shell -> child process -> effect`. The
missing layer is not more capture. It is a **semantic aggregation key** that is
cheap enough to run locally, stable enough to compare across runs, and weak
enough not to become an untrusted judge.

The proposed answer is deliberately constrained:

```text
small model: one-word semantic tags
AgentSight:  exact provenance and metrics
query layer: prompt-tagged footprint aggregation
views:       session split, footprint table, system flamegraph, token flamegraph, behavior diff
```

## 2. Non-Goals

This research plan explicitly does not claim:

- task completion detection;
- code correctness;
- validation sufficiency;
- safety or compliance judgment;
- general claim truth checking;
- malicious behavior detection.

Those require domain-specific oracles, policies, or human review. One-word tags
are only grouping keys.

## 3. System Model

### 3.1 Inputs

Agent-native semantic input:

- session metadata;
- user turns;
- assistant turns;
- tool calls;
- subagent spawn/wait/close events;
- LLM call metadata and token usage.

AgentSight system input:

- process nodes;
- tool-call to shell/process provenance;
- file read/write/delete/create effects when covered;
- network destinations;
- resource samples;
- command exit status;
- capture coverage metadata.

### 3.2 One-Word Tag Contract

The small model outputs open-vocabulary tags, but every accepted tag must obey:

```text
lowercase
ascii
single word
no spaces
no underscore
no hyphen
max 16 chars
no verdict words
no safe/unsafe/pass/fail
```

Examples:

```text
paper
review
revise
draft
audit
debug
test
inspect
edit
docs
collector
frontend
flamegraph
coverage
baseline
harness
```

Invalid outputs are rejected or retried. Accepted tags are then canonicalized
with simple rules and optional repo dictionaries:

```text
revision -> revise
updated -> revise
reviews -> review
testing -> test
investigate -> inspect
documentation -> docs
```

### 3.3 Tagged Entities

```text
SessionTag   whole session or episode name
SubagentTag  subagent role or activity
TaskTag      user-requested work atom
PromptTag    user turn or prompt cluster
LlmCallTag   model-call activity inside a turn
```

Tags are upper frames and grouping hints. They are not verdicts.

### 3.4 Effect Attachment States

Because AgentSight has exact provenance, captured effects normally attach to a
prompt/tool/subagent. The useful states are:

| State | Meaning |
| --- | --- |
| `attached` | effect follows provenance to a prompt/tool/subagent |
| `background` | attached but semantically incidental, such as shell startup or package-manager children |
| `longlived` | process started by a prompt but continues across prompts |
| `coverage` | capture is missing, late, ambiguous, or dropped |

Only coverage gaps should produce unknown attachment. A complete captured effect
should not be called unlinked.

### 3.5 Stack Grammars

System footprint flamegraph:

```text
session_tag ; agent ; subagent_tag ; prompt_tag ; tool_kind ; command ; process ; effect ; status
```

Token/reasoning flamegraph:

```text
session_tag ; agent ; subagent_tag ; prompt_tag ; llm_call_tag ; model ; status
```

The stack expresses execution ownership and causal nesting. Tags are only the
upper semantic frames.

### 3.6 Formal Data Model

The research object is a tagged agent trajectory:

```text
Trajectory = (S, P, A, T, E, R, C)
```

where:

- `S` is a session or episode;
- `P` is the ordered set of user prompts or prompt clusters;
- `A` is the set of agents and subagents;
- `T` is the set of tool calls;
- `E` is the set of exact system effects;
- `R` is the exact provenance relation from prompts/tools/processes to effects;
- `C` is the set of coverage facts.

The semantic tagger is a lossy naming function:

```text
tag(entity) -> one_word | unknown
```

The tagger does not change `R`. It only adds grouping keys. The query layer
computes:

```text
group_by(tag(prompt), stack_signature(effect)) over E
```

This distinction is the core safety property:

```text
Tags may be wrong, but provenance must remain intact.
```

### 3.7 Required Invariants

The implementation must satisfy these invariants before any user study or
paper result counts:

| Invariant | Meaning | Checker |
| --- | --- | --- |
| I1 Tag contract | accepted tags are lowercase single words under 16 chars | tag validator |
| I2 Provenance preservation | removing tags does not remove or alter effects | snapshot diff |
| I3 Effect conservation | every covered effect appears in exactly one stack or coverage bucket | stack audit |
| I4 Metric conservation | summed stack widths equal source metric totals per run | aggregation audit |
| I5 Authority separation | tags cannot create, delete, or relabel system facts | type/schema check |
| I6 Coverage honesty | attach gaps produce `coverage`, not silent attachment | injected fault test |

These invariants are stronger than UI correctness. They make the semantic layer
auditable as a systems component.

## 4. System-Under-Test Model

Components:

- AgentSight capture and materialized view;
- local session parser for agent-native transcripts;
- one-word tagger and canonicalizer;
- provenance joiner;
- stack generator;
- report/web renderers;
- optional diff engine.

Durable state:

- existing SQLite/session snapshots;
- optional `SemanticTagRow` and `TaggedEffectRow`;
- folded stack files and rendered SVG/HTML reports.

Trust/failure boundaries:

- system effects and provenance are authoritative within coverage;
- tags are untrusted grouping keys;
- coverage gaps are explicit and must not be hidden by tags;
- subagent boundaries are semantic/execution ownership boundaries;
- long-lived processes retain their original owning prompt and get a
  `longlived` status.

Guarantees claimed:

- no correctness, safety, or task-completion guarantee;
- conservation of covered effects in aggregated views;
- explicit coverage state when provenance is incomplete;
- tag removal preserves exact system evidence.

Assumptions:

- AgentSight capture provides tool-to-effect provenance for the evaluated runs;
- agent-native sessions expose enough prompt/tool/subagent structure to tag;
- redaction preserves enough text for tag generation in the experimental corpus.

## 5. Claim Ledger

| ID | Claim | Scope | Metric/evidence needed | Status |
| --- | --- | --- | --- | --- |
| C1 | One-word tags can provide useful semantic grouping without becoming an unreliable judge. | Coding-agent sessions with prompts, tool calls, subagents, and LLM calls. | Tag contract validity, human semantic adequacy, stability, latency/cost. | planned |
| C2 | Joining tags with exact provenance produces prompt-tagged system footprints that transcript-only and system-only tools cannot produce automatically. | Captured effects with complete provenance in local CLI agent runs. | Attachment completeness, aggregation correctness, analysis task accuracy/time vs baselines. | planned |
| C3 | Task-aligned system and token flamegraphs reveal effort concentration, loops, subagent cost, and behavior differences better than trace trees or raw timelines. | Single-run and cross-run analysis over coding-agent workloads. | User/reviewer task accuracy, time-to-answer, loop/diff detection F1, compression ratio. | planned |
| C4 | The design degrades safely under ambiguous tags, long-lived children, concurrent agents, and coverage gaps. | Robustness workloads with injected ambiguity and capture degradation. | Invariant checks: no dropped covered effects, coverage state correctness, long-lived classification. | planned |
| C5 | Local tagging and aggregation are practical for long sessions and cross-session analysis. | Runs with 10-1,000 turns and many effects. | Tag latency, query latency, storage growth, CPU/memory overhead, tail latency. | planned |
| C6 | The resulting behavior profiles reveal real agent workflow structure across sessions. | Multi-session corpora from coding-agent workloads. | Distribution shifts, subagent cost/effect ratios, loop frequencies, case-study evidence. | planned |

## 6. Claim-To-Experiment Map

| Claim | Required evidence | Primary block | Falsifying result | Supported wording if partial |
| --- | --- | --- | --- | --- |
| C1 | Tags satisfy contract and match human semantic grouping often enough to be useful. | B1 Tag quality | Small tags are unstable, invalid, or semantically useless. | Tags are useful only for a small subset of prompt types. |
| C2 | Prompt-tagged footprints improve analysis tasks over transcript-only/system-only baselines. | B2 End-to-end utility | Baselines match accuracy and time-to-answer. | Benefit limited to multi-tool or subagent-heavy sessions. |
| C3 | Flamegraphs reveal loops, heavy phases, and behavior diffs better than alternatives. | B3 Flamegraph/diff | Trace tree or raw table performs equally well. | Flamegraph helps for cross-run diff but not single-run inspection. |
| C4 | Coverage and long-lived cases degrade to explicit states without dropping or misattaching effects. | B4 Robustness | Covered effects are dropped or misattached without coverage indication. | Robust only when capture starts before the agent run. |
| C5 | Overhead is acceptable at realistic scale. | B5 Performance/scale | Tagging/query latency or storage growth makes default use impractical. | Usable as offline report, not live top. |
| C6 | Cross-session profiles expose workflow structure and behavior differences. | B6 Real-session study | Profiles show no reproducible patterns beyond raw event counts. | Case-study claim only. |

## 7. Components

1. **Session parser**

   Parses local Claude/Codex-style sessions into sessions, prompts, assistant
   turns, tool calls, subagent events, LLM calls, and token rows.

2. **One-word tagger**

   Runs a local small model or a model API in controlled mode. It outputs only
   one word per entity. It cannot emit findings or modify provenance.

3. **Tag validator and canonicalizer**

   Enforces the tag contract and merges obvious variants.

4. **Provenance joiner**

   Joins prompt/tool/subagent entities to exact system effects through existing
   AgentSight provenance.

5. **Footprint aggregator**

   Aggregates processes, file effects, network destinations, resource samples,
   command exits, token usage, and duration by semantic tag and execution stack.

6. **Stack generator**

   Emits folded stacks for system footprint and token footprint separately.

7. **Diff engine**

   Compares two runs or two cohorts using normalized stack signatures and metric
   deltas.

8. **Views**

   - Session Split
   - Prompt-Tag Footprint Table
   - System Footprint Flamegraph
   - Token / Reasoning Flamegraph
   - Behavior Diff Flamegraph

## 8. Trust And Failure Boundaries

The small model is not trusted. It provides a tag candidate only.

Authoritative sources:

- exact provenance;
- command exit status;
- observed file/network/resource effects;
- token rows;
- coverage metadata.

Untrusted or weak sources:

- one-word tag semantic adequacy;
- tag canonicalization;
- inferred episode boundaries.

Failure handling:

| Failure | Required behavior |
| --- | --- |
| invalid tag | reject or retry; fallback to `unknown` |
| low tag confidence | keep provenance, use `unknown` tag |
| prompt boundary ambiguity | keep both candidate groupings or mark episode ambiguous |
| long-lived child | preserve parent prompt but mark `longlived` |
| concurrent agents | attach by exact provenance; if ambiguous, mark `coverage` |
| capture gap | preserve available data and mark affected metrics as `coverage` |

## 9. Analysis Tasks

The core evaluation tasks should be query-like and answerable from ground truth,
not subjective impressions.

| Task ID | Question | Expected answer source |
| --- | --- | --- |
| Q1 | Which prompt tag produced the most file writes? | generated prompt-to-effect map |
| Q2 | Which prompt tag produced the most failed command exits? | command exit rows |
| Q3 | Did a subagent review phase precede later revision writes? | subagent timestamp + write rows |
| Q4 | Which prompt launched a long-lived process? | long-lived process oracle |
| Q5 | Which stack signature grew most between two runs? | injected behavior diff |
| Q6 | Which prompt tag dominates token cost but has little system effect? | token/effect ratio |
| Q7 | Did coverage degradation affect any aggregated metric? | injected coverage fault |

These tasks define the main user-study and automated benchmark outputs. A
baseline must answer the same task set to count as a fair comparison.

## 10. Experiment Matrix

| Block | Claim | Experiment | Baselines/variants | Metrics | Oracle | Figure/table | Priority |
| --- | --- | --- | --- | --- | --- | --- | --- |
| B1 | C1 | Tag quality and stability | no tags, fixed taxonomy, larger model, free-form summary | validity, agreement, stability, latency/cost | human tag adequacy + contract checker | Fig. 2 / Table 2 | must |
| B2 | C2 | End-to-end analysis tasks | transcript-only, system-only, trace tree, manual join, free-form LLM summary | accuracy, time-to-answer, confidence, errors | task answer key from ground truth | Fig. 3 | must |
| B3 | C3 | Flamegraph and behavior diff utility | process flamegraph, token dashboard, timeline, fixed taxonomy flamegraph | loop/diff detection F1, compression, time | injected loops/diffs + human labels | Fig. 4 / Fig. 5 | must |
| B4 | C4 | Robustness and degradation | no coverage model, no long-lived state, no canonicalizer | invariant pass rate, false attachment, dropped effects | provenance checker + injected faults | Fig. 6 | must |
| B5 | C5 | Performance and scale | no semantic layer, larger model, offline-only mode | p50/p95/p99 latency, CPU, memory, storage | measurement scripts | Fig. 7 | must |
| B6 | C6 | Real-session behavior profiling | current AgentSight report, raw logs | profile stability, observed patterns | reproducible session artifacts | Table 3 | should |

## 11. Experiment Blocks

### B1. Tag Quality And Stability

- Claim tested: C1.
- Hypothesis: a constrained one-word tagger gives enough semantic grouping for
  aggregation while staying cheap and reproducible.
- Why this block exists: reviewers will ask whether tags are meaningful or just
  arbitrary LLM noise.
- Workload:
  - 300-500 prompt/LLM-call/subagent snippets from local AgentSight sessions;
  - at least 6 task families: implement, debug, test, review, docs, cleanup;
  - at least 3 agents or agent surfaces when available.
- Compared systems:
  - one-word local small model;
  - larger model with same one-word contract;
  - fixed taxonomy classifier;
  - free-form LLM summary;
  - no semantic tags.
- Metrics:
  - tag contract validity rate;
  - semantic adequacy against two human raters;
  - inter-rater agreement;
  - tag stability across repeated runs;
  - canonicalization collision rate;
  - tag latency and cost.
- Oracle:
  - contract validator for syntax;
  - human binary adequacy label: “does this one word preserve the main activity
    needed for grouping?”
  - cluster purity for known task families.
- Success criterion:
  - accepted tags satisfy contract at 99%+ after one retry;
  - adequacy is close to the larger model and better than fixed taxonomy;
  - local tagging latency is low enough for report-time use.
- Failure interpretation:
  - if tags are unstable, restrict to prompt-level tags only;
  - if open vocabulary drifts too much, add per-repo dictionaries or a small
    allowed vocabulary per component.
- Figure/table target:
  - Table: tag validity, adequacy, stability, latency.
  - Figure: tag distribution and confusion/collision examples.
- Reproducibility artifacts:
  - `tag_inputs.jsonl`, `tag_outputs.jsonl`, human label sheet, tagger config.

### B2. End-To-End Prompt Footprint Utility

- Claim tested: C2.
- Hypothesis: prompt-tagged footprints let users answer behavior questions
  faster and more accurately than transcript-only or system-only views.
- Why this block exists: it proves the semantic layer matters beyond aesthetics.
- Workload:
  - 60 controlled sessions with known prompt-to-effect structure;
  - 40 real sessions from AgentSight/Codex/Claude-style local histories.
- Analysis tasks:
  - identify which prompt phase caused most file writes;
  - identify which prompt phase caused network activity;
  - identify whether review subagent activity led to later revision writes;
  - identify which prompt caused long-lived children;
  - identify repeated command loops under a prompt tag.
- Compared systems:
  - transcript-only view;
  - system-only process/file/network table;
  - generic timeline;
  - current intent-effect trace tree;
  - free-form LLM summary;
  - AgentSight with one-word tags.
- Metrics:
  - task answer accuracy;
  - time-to-answer;
  - wrong-answer rate;
  - analyst confidence;
  - number of drill-downs.
- Oracle:
  - controlled sessions have generated answer keys;
  - real sessions use two-rater adjudication.
- Success criterion:
  - AgentSight improves time-to-answer and accuracy on at least three analysis
    tasks without increasing false confidence.
- Failure interpretation:
  - if only certain tasks improve, scope claim to those tasks.
- Figure/table target:
  - Figure: task accuracy/time vs baseline.
  - Table: examples of questions answered only by semantic+system join.
- Reproducibility artifacts:
  - session DBs, transcripts, answer keys, UI screenshots, study protocol.

### B3. System And Token Flamegraph Utility

- Claim tested: C3.
- Hypothesis: execution-stack flamegraphs with one-word upper frames expose
  effort concentration, loops, and behavior differences better than process
  flamegraphs or raw timelines.
- Why this block exists: this is the core visualization contribution.
- Workload:
  - controlled sessions with injected loops and behavior changes;
  - paired runs across model/prompt/workflow versions;
  - real sessions with review/revision and test/debug loops.
- Compared systems:
  - process-only flamegraph;
  - token-only dashboard;
  - causal timeline;
  - trace tree;
  - fixed-taxonomy flamegraph;
  - task-aligned system/token flamegraphs.
- Metrics:
  - loop detection F1;
  - behavior-diff detection F1;
  - compression ratio from events to stack signatures;
  - time-to-localize dominant footprint;
  - qualitative case study evidence.
- Oracle:
  - injected loops/diffs in controlled workloads;
  - human-labeled loop/diff annotations for real sessions.
- Success criterion:
  - task-aligned flamegraphs outperform process-only and token-only views for
    detecting prompt-level loops and cross-run behavior changes.
- Failure interpretation:
  - if flamegraphs help only in cross-run analysis, make behavior diff the main
    visualization claim and keep single-run flamegraph as secondary.
- Figure/table target:
  - Figure: system footprint flamegraph.
  - Figure: token flamegraph.
  - Figure: behavior diff flamegraph.
- Reproducibility artifacts:
  - folded stacks, SVGs, source sessions, rendering config.

### B4. Robustness And Safe Degradation

- Claim tested: C4.
- Hypothesis: the design preserves exact provenance and degrades to explicit
  states under tag or capture uncertainty.
- Why this block exists: reviewers will attack ambiguous prompts, background
  tools, long-lived children, and concurrent agents.
- Workload:
  - prompt ambiguity;
  - deliberately bad tags;
  - canonicalization collisions;
  - long-lived server processes;
  - package-manager and shell background activity;
  - concurrent agents;
  - late attach and partial capture.
- Compared variants:
  - full system;
  - no canonicalizer;
  - no long-lived state;
  - no coverage model;
  - no subagent frame;
  - prompt-only tagging.
- Metrics:
  - covered effect preservation rate;
  - false attachment rate;
  - dropped effect count;
  - correct `background`/`longlived`/`coverage` classification;
  - tag-induced grouping error.
- Oracle:
  - generated provenance ground truth in controlled runs;
  - invariants:
    - every covered effect appears in exactly one stack or coverage bucket;
    - removing tags never removes provenance;
    - long-lived children are not silently attributed to later prompts.
- Success criterion:
  - invariants hold across robustness cases, even when tag quality degrades.
- Failure interpretation:
  - if concurrent agents are too noisy, scope claims to runs with reliable
    process/provenance boundaries and mark concurrent ambiguity as coverage.
- Figure/table target:
  - Figure: degradation matrix.
  - Table: failure modes and outputs.
- Reproducibility artifacts:
  - fault injection scripts, expected invariant outputs.

### B5. Performance And Scale

- Claim tested: C5.
- Hypothesis: local one-word tagging and aggregation are practical for normal
  coding-agent reports and offline cross-session analysis.
- Why this block exists: small semantic models are only useful if they are cheap
  enough to run by default.
- Workload:
  - synthetic sessions with 10, 50, 100, 500, and 1,000 turns;
  - real sessions with thousands of tool/process/effect rows;
  - cross-session aggregation over 10, 100, and 1,000 sessions.
- Compared systems:
  - no semantic layer;
  - local small model;
  - larger model;
  - free-form summarizer;
  - cached tags vs cold tags.
- Metrics:
  - tag latency p50/p95/p99;
  - report query latency p50/p95/p99;
  - CPU and memory;
  - SQLite/storage growth;
  - flamegraph generation time;
  - cache hit sensitivity.
- Oracle:
  - measurement scripts with fixed session corpus and machine metadata.
- Success criterion:
  - default report path remains interactive for common sessions;
  - cross-session aggregation remains feasible offline.
- Failure interpretation:
  - if live use is too slow, claim offline report only.
- Figure/table target:
  - latency CDFs, storage growth, scaling curves.
- Reproducibility artifacts:
  - benchmark script, corpus manifest, machine config.

### B6. Real-Session Case Studies

- Claims tested: C1-C3 qualitatively.
- Hypothesis: the plan exposes useful real patterns such as review/revision
  loops, token-heavy subagents, repeated test loops, and behavior shifts.
- Workload:
  - this AgentSight research-plan session;
  - several existing Codex/Claude sessions from this repository;
  - optional sessions from other open-source repos.
- Compared systems:
  - raw transcript;
  - raw AgentSight report;
  - one-word tagged aggregation.
- Metrics:
  - case-study evidence, not headline quantitative claims.
- Oracle:
  - raw session artifacts and exact provenance.
- Figure/table target:
  - a compact table of observed behavior patterns with stack/evidence links.

## 12. Baseline Fairness

Named baselines:

| Baseline | What it sees | What it proves |
| --- | --- | --- |
| transcript-only | prompt, assistant text, tool calls | whether semantic text alone is enough |
| system-only | process/file/network/resource rows | whether OS facts alone are enough |
| trace tree | exact causal tree without collapsed aggregation | whether aggregation adds value |
| process flamegraph | command/process stacks only | whether semantic upper frames matter |
| token dashboard | model/token/cost by turn | whether system footprint adds value |
| fixed taxonomy | predefined labels like inspect/edit/test | whether open tags matter |
| free-form LLM summary | natural-language report | whether constrained tags beat summaries |
| manual join | human gets transcript + system rows | whether automation reduces time/errors |

Tuning policy:

- baselines get the same session corpus;
- free-form summaries get the same model budget across runs;
- manual join gets fixed time budgets: 4, 8, and 16 minutes per session;
- each baseline outputs answers to the same analysis tasks, not just screenshots.

Intentionally omitted:

- security-only tools as primary baselines, because the claim is behavior
  aggregation rather than threat detection;
- domain-specific correctness checkers, because correctness is out of scope.

## 13. Workloads

Controlled workloads:

- 3-5 small or medium open-source repos;
- task templates:
  - implement feature;
  - debug failing test;
  - docs/research writing;
  - review/revision;
  - cleanup/refactor;
  - dependency update;
- injected patterns:
  - repeated command loop;
  - review/revision loop;
  - token-heavy subagent;
  - long-lived server;
  - behavior diff between two prompts;
  - coverage gap.

Real workloads:

- local AgentSight sessions;
- Codex/Claude CLI sessions in this repo;
- optional sessions from other repos if privacy allows.

Stress workloads:

- long session with 1,000 turns;
- many subagents;
- concurrent agents;
- noisy package-manager children;
- tag drift and invalid tag outputs;
- attach late / partial capture.

## 14. Metrics

Tag metrics:

- contract validity;
- semantic adequacy;
- repeated-run stability;
- canonicalization collision rate;
- tag latency and cost.

Aggregation metrics:

- effect preservation rate;
- attachment completeness;
- compression ratio;
- stack cardinality;
- prompt-level footprint accuracy.

Visualization/user metrics:

- task answer accuracy;
- time-to-answer;
- loop/diff detection F1;
- number of drill-downs;
- analyst confidence.

Robustness metrics:

- false attachment;
- dropped effects;
- correct `background`/`longlived`/`coverage` state;
- invariant pass rate.

Performance metrics:

- tag p50/p95/p99 latency;
- report p50/p95/p99 query latency;
- CPU/memory overhead;
- storage growth;
- flamegraph render time.

## 15. Run Order

| Run ID | Stage | Purpose | Config | Seed/reps | Decision gate | Cost | Risk |
| --- | --- | --- | --- | --- | --- | --- | --- |
| R001 | sanity | validate tag contract on 20 snippets | local small model, no canonicalizer | 3 reps | 99% valid after retry | low | invalid outputs |
| R002 | sanity | generate folded stacks for one real session | one AgentSight session | 1 | every covered effect appears in a stack | low | provenance gaps |
| R003 | baseline | run transcript/system/fixed taxonomy baselines | 10 controlled sessions | 3 reps | baselines produce task answers | medium | unfair baseline setup |
| R004 | main | B1 tag quality on full corpus | 300-500 snippets | 3 reps | adequacy and stability meet threshold | medium | tag drift |
| R005 | main | B2 user utility | 60 controlled + 40 real sessions | per study design | accuracy/time improvement | high | user study cost |
| R006 | main | B3 flamegraph/diff | injected loops/diffs | 3 reps | loop/diff F1 improves | medium | weak visualization signal |
| R007 | robustness | B4 degradation | noisy/long-lived/concurrent/capture gap | 3 reps | invariants pass | medium | false attachments |
| R008 | scale | B5 performance | 10-1,000 turns, 10-1,000 sessions | 5 reps | latency/storage acceptable | medium | local model slow |
| R009 | polish | case studies | selected real sessions | 1 | evidence-backed examples | low | anecdotal only |

## 16. Claim Gates

Before paper writing, claims must pass these gates:

```text
G1: accepted tags satisfy the one-word contract at >=99% after one retry.
G2: removing tags does not remove or alter exact provenance.
G3: every covered effect appears in exactly one stack or explicit coverage bucket.
G4: AgentSight beats at least two strong baselines on time-to-answer or accuracy for prompt-footprint tasks.
G5: task-aligned flamegraphs beat process-only flamegraphs for at least one loop/diff detection workload.
G6: performance remains interactive for normal single-session reports.
```

If a gate fails:

- G1 failure: use fixed or per-repo constrained vocabulary instead of open tags.
- G4 failure: narrow contribution to cross-session behavior profiling.
- G5 failure: demote flamegraph to secondary visualization and lead with
  Prompt-Tag Footprint Table.
- G6 failure: make the system offline/report-only.

## 17. Negative Result Handling

| Negative result | Claim revision |
| --- | --- |
| tags are too unstable | claim only canonicalized/fixed-tag grouping |
| small model much worse than larger model | use larger model as optional high-quality mode, keep local model as cheap mode |
| free-form summary performs well | focus on reproducibility, folded-stack aggregation, and exact provenance preservation |
| system-only baseline performs well | scope to tasks requiring semantic prompt/subagent grouping |
| flamegraph not useful for single runs | focus on behavior diff and cross-session aggregation |
| user study shows low benefit | reposition as measurement/tooling paper with quantitative behavior profiles |

## 18. Reproducibility

Artifacts:

- session corpus manifest;
- raw transcripts with redaction policy;
- AgentSight DBs or snapshots;
- tag input/output JSONL;
- canonicalizer config;
- folded stack files;
- SVG/HTML outputs;
- user-study tasks and answer keys;
- benchmark scripts;
- result tables.

Result path convention:

```text
results/semantic-tags/<date>/<run-id>/
  corpus.json
  tags.jsonl
  folded-system.txt
  folded-token.txt
  metrics.json
  figures/
```

Required tracker columns:

```text
Run ID, Claim, Block, Purpose, Config, Commit, Machine, Seed/Reps,
Oracle, Decision Gate, Result Path, Status
```

## 19. Industrial And Academic Value

Industrial value:

- local and privacy-preserving behavior summaries for coding-agent sessions;
- prompt-level cost and footprint accounting for agent users and platform teams;
- subagent cost/effect analysis for workflow designers;
- behavior regression testing across model, prompt, tool, or skill versions;
- lower-friction reports than raw traces or full transcript summaries.

Academic value:

- defines a constrained semantic index for agent trajectories;
- separates untrusted semantic naming from authoritative system provenance;
- introduces conservation-style invariants for semantic aggregation views;
- evaluates task-aligned flamegraphs and behavior diffs as systems artifacts;
- creates a reproducible benchmark for prompt-tagged system footprint analysis.

The top-conference contribution is not that a model can name prompts. The
contribution is the systems construction around that weak signal: tag contract,
canonicalization, provenance preservation, metric conservation, stack grammar,
robust degradation, and comparison against strong transcript/system baselines.

## 20. Residual Uncertainty

This plan does not prove:

- agent correctness;
- task completion;
- safety;
- general semantic understanding;
- universality across all agent frameworks.

This is acceptable because the paper claim is narrower: semantic tags are a
cheap grouping layer for system behavior aggregation, not a correctness oracle.

The highest research risk is whether one-word tags are simultaneously stable
enough for aggregation and expressive enough to help real analysis tasks.

## 21. Implementation Handoff

Minimum implementation slice:

1. Build a one-word tagger wrapper with strict output validation.
2. Tag session, prompt, subagent, and LLM call rows.
3. Canonicalize simple variants.
4. Join tags to existing exact provenance.
5. Emit `TaggedEffectRow` in memory.
6. Render Prompt-Tag Footprint Table.
7. Generate system folded stacks.
8. Generate token folded stacks.
9. Add behavior diff for two folded-stack files.
10. Run R001-R003 before expanding the paper story.

Do not implement verdicts, claim checking, or validation sufficiency in this
research path. Those are separate systems with stronger oracle requirements.
