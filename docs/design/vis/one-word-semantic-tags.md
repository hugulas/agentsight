# One-Word Semantic Tags And Task-Aligned Aggregation

状态：设计草案

这份文档定义 AgentSight 的轻量 semantic layer：用本地小模型把
session、subagent、task、prompt、LLM call 命名成一个词，然后把这些词作为
系统聚合和可视化的上层 frame。系统事实仍然来自 AgentSight 的精确
provenance，不由模型判断。

## 结论

小模型只做命名，不做裁判。

```text
small model:  prompt / turn / subagent / llm call -> one word
AgentSight:   tool_call -> shell -> child process -> file/network/resource effect
query layer:  join tags with exact provenance, then aggregate
```

这层 semantic tag 能解决的问题是行为聚合和对齐：

- 这个 session 是否围绕同一个工作展开？
- 哪类 prompt 产生最多系统 footprint？
- subagent 消耗了多少 token，是否引出后续 revision/write？
- 哪些 prompt/tag 导致 long-lived process、network、failed exit 或大量 writes？
- agent 是否在 `read -> edit -> test -> edit -> test` 这类 loop 里反复消耗成本？
- 两个 agent/model/prompt 的行为 footprint 有什么差异？

它不能可靠解决的问题：

- 任务是否完成；
- 代码是否正确；
- 测试是否充分；
- agent claim 是否真实；
- 行为是否安全或合规。

这些判断如果需要做，只能由明确 oracle、policy 或人工 review 支撑。tag 本身只用于命名、聚合、排序和展示。

## One-Word Tag Contract

小模型输出不是固定集合，但必须是单词。

约束：

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

好标签：

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

坏标签：

```text
osdi_review        # contains underscore, too specific
checker-spec       # contains hyphen
task completed     # phrase and verdict
unsafe             # verdict
tests_failed       # verdict plus underscore
```

如果模型输出不满足格式，系统把 tag 置为 `unknown`，或者重新 ask 一次。

## Tag Scope

不同层级都可以有一个词，但每个词只描述该层的主要语义。

```text
SessionTag   whole session or episode name
SubagentTag  subagent role
TaskTag      user-requested work atom
PromptTag    one user turn / prompt cluster
LlmCallTag   model-call activity inside a turn
```

例子：

```text
session:  paper
task:     draft
prompt:   revise
llmcall:  plan
subagent: review
```

不要让一个 tag 承载完整语义。复杂语义由多个 stack frame 组合表达：

```text
paper;main;revise;patch;apply;file;ok
```

而不是：

```text
osdi_checker_spec_revision
```

## Not A Fixed Taxonomy

tag 不是固定 enum。固定集合会太硬，跨 repo、跨 task、跨 agent 时表达力不够。

但自由输出会带来同义词和漂移，因此需要 canonicalization：

```text
revision -> revise
updated  -> revise
reviews  -> review
testing  -> test
investigate -> inspect
documentation -> docs
```

canonicalizer 可以分三层：

1. 纯规则：lowercase、strip punctuation、长度限制、简单 stemming。
2. repo dictionary：`collector`, `frontend`, `bpf`, `docs`, `cargo`, `pytest`。
3. 小模型重写：当 tag 太长或太怪时，要求重新输出一个单词。

重要边界：

```text
tag is a grouping key, not an authority.
```

## Provenance Premise

AgentSight 已经能精确连接：

```text
tool_call -> shell -> child process -> file/network/resource effect
```

因此正常情况下，不应该说 captured system effects “无法归到 prompt”。只要 tool call 属于某个 assistant turn / prompt episode，effect 就能沿 provenance 回到上层 prompt tag。

更准确的 effect 状态是：

| 状态 | 含义 | 展示 |
| --- | --- | --- |
| `attached` | effect 沿 provenance 归到 prompt/tool/subagent | 默认聚合 |
| `background` | effect 归得上，但属于工具、shell、package manager、language server 等附带行为 | 单独色块或折叠 |
| `longlived` | prompt 启动的 server/daemon 持续跨 prompt 运行 | 独立 lane 或 stack frame |
| `coverage` | capture 缺失、attach late、detached ownership 不清 | coverage overlay |

只有 coverage 不足时才输出 `unknown`。不要把已 captured 且可追溯的 effect 叫 `unlinked`。

## Data Flow

```text
agent transcript / local session
  -> episode and prompt boundaries
  -> one-word semantic tagger

system capture / materialized view
  -> tool calls
  -> process tree
  -> file/network/resource effects

join
  -> prompt-tagged effect rows
  -> aggregations
  -> visualizations
```

推荐派生 rows：

```text
SemanticTagRow
  id
  entity_kind        # session | episode | subagent | task | prompt | llm_call
  entity_id
  tag
  raw_output
  source_model
  confidence?

TaggedEffectRow
  effect_id
  prompt_id
  prompt_tag
  subagent_id?
  subagent_tag?
  tool_call_id
  process_node_id?
  effect_kind
  status
  metric_value
```

MVP 可以先不落 SQLite 表，在 `report`/web query 层派生。

## Small Model Prompt Shape

模型输入要短，输出只允许一个词。

```text
Name this agent activity with one lowercase ascii word.
No spaces. No underscore. No hyphen. Max 16 chars.
Do not judge correctness or safety.

Text:
根据 subagent 的 OSDI review 修改论文草稿，补充 checker specs 和 evaluation protocol

Tag:
```

期望输出：

```text
revise
```

对 LLM call：

```text
Text:
The assistant compares the review comments and decides what sections to patch next.

Tag:
```

输出：

```text
plan
```

## Visualization 1: Session Split

解决的问题：

```text
这个 session 里有几个 episode？
当前 prompt 是继续上一件事，还是开启新工作？
subagent 是在哪个 episode 里出现的？
```

形态：

```text
time ─────────────────────────────────────────────────────────>

[ask]paper [read]inspect [write]draft [agent]review [edit]revise [agent]review [edit]revise [audit]audit
└──────────────────────────── Episode: paper ───────────────────────────────────────────────────────┘
```

如果换任务：

```text
[fix]debug [run]test [edit]fix       [ask]paper [write]draft [agent]review
└──── Episode: debug ────┘            └────── Episode: paper ───────────┘
```

需要的数据：

- user turn timestamps;
- assistant turn timestamps;
- subagent spawn/wait/close events;
- one-word prompt/session/subagent tags;
- optional cwd/repo change signals.

不需要展示：

- full prompt text;
- raw model output;
- every tool call.

## Visualization 2: Prompt-Tag Footprint Table

解决的问题：

```text
每类 prompt tag 造成了多少系统 footprint？
哪些 prompt 只是聊天，哪些 prompt 真的动了系统？
哪些 prompt 产生 long-lived processes 或大量 background effects？
```

形态：

| Prompt tag | Tools | Processes | File writes | Network | Tokens | Time | Long-lived |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| inspect | 28 | 31 | 0 | 0 | 8k | 2m10s | 0 |
| draft | 4 | 0 | 1 | 0 | 12k | 3m20s | 0 |
| review | 3 | 0 | 3 | 0 | 11k | 5m40s | 0 |
| revise | 9 | 0 | 4 | 0 | 10k | 4m10s | 0 |
| audit | 5 | 5 | 0 | 0 | 1k | 0m30s | 0 |

这个表不做 verdict，只做 footprint 聚合。

## Visualization 3: System Footprint Flamegraph

解决的问题：

```text
系统 effort 聚合在哪里？
哪些 command/process/effect 重复最多？
subagent 或 prompt tag 的系统 footprint 有多大？
```

这里的 stack 必须表达 execution ownership / causal nesting，而不是任意语义面包屑。

Stack grammar：

```text
session_tag ; agent ; subagent_tag ; prompt_tag ; tool_kind ; command ; process ; effect ; status
```

字段说明：

| Frame | 来源 |
| --- | --- |
| `session_tag` | one-word tag |
| `agent` | main / agent id |
| `subagent_tag` | one-word tag or `none` |
| `prompt_tag` | one-word tag |
| `tool_kind` | exec / patch / read / agent / mcp |
| `command` | normalized command, such as `rg`, `sed`, `cargo`, `apply` |
| `process` | normalized process name |
| `effect` | file / network / resource / stdout / exit |
| `status` | ok / fail / background / longlived / coverage |

Example folded stacks from the paper-design session:

```text
paper;main;none;inspect;exec;sed;sed;file;ok          20
paper;main;none;inspect;exec;rg;rg;file;ok            12
paper;main;none;draft;patch;apply;writer;file;ok       1
paper;main;review;review;agent;wait;model;token;ok     3
paper;main;none;revise;patch;apply;writer;file;ok      5
paper;main;none;audit;exec;git;git;stdout;ok           2
```

Width can be:

```text
duration
process count
file event count
changed lines
stdout bytes
failed exits
effect weight
```

Color:

| Color | Meaning |
| --- | --- |
| blue | model/subagent/tool control |
| green | file/process effect |
| yellow | repeated/background/longlived |
| red | failed exit |
| gray | coverage/unknown |

This is a real flamegraph because identical stack signatures collapse and widths add up.

## Visualization 4: Token / Reasoning Flamegraph

解决的问题：

```text
token、latency、cost 花在哪里？
主 agent 和 subagent 各自占多少？
review/revision loop 的 token 成本有多大？
```

System footprint 和 token footprint 不应该硬塞进一张图，因为单位不同。

Stack grammar：

```text
session_tag ; agent ; subagent_tag ; prompt_tag ; llm_call_tag ; model ; status
```

Example:

```text
paper;main;none;draft;write;gpt;ok          12000
paper;main;review;review;critique;gpt;ok     5000
paper;main;none;revise;plan;gpt;ok           3000
paper;main;none;revise;write;gpt;ok          7000
paper;main;review;review;critique;gpt;ok     3500
paper;main;none;audit;summarize;gpt;ok       1000
```

Width:

```text
input tokens
output tokens
total tokens
latency
cost
```

This answers whether subagents are cheap reviewers, expensive critics, or cost centers that do not change the system footprint.

## Visualization 5: Behavior Diff Flamegraph

解决的问题：

```text
两个 agents / prompts / model versions 的行为 footprint 有什么差异？
新版本是否更 review-heavy、test-heavy、network-heavy、write-heavy？
```

Use the same stack grammar as the system or token flamegraph, then render diff width:

```text
delta = current_metric - baseline_metric
```

Example:

```text
codex:
paper;main;none;inspect;exec;rg;rg;file;ok       ██████
paper;main;none;revise;patch;apply;writer;file   ████████

claude:
paper;main;none;inspect;exec;rg;rg;file;ok       ███████████
paper;main;none;review;llm;critique;model;token  ███████
```

This is the right surface for regression testing:

- did a new model use more shell commands?
- did a new prompt create more file writes?
- did adding a review subagent reduce later edit churn?
- did a new workflow introduce long-lived processes?

## What The Small Model Enables

With only one-word tags, AgentSight can analyze:

| Analysis | What it uses |
| --- | --- |
| prompt footprint | prompt tag + exact effects |
| subagent contribution | subagent tag + token/system metrics |
| review loop | sequence of `review` and `revise` tags |
| command loops | repeated command stacks under same prompt tag |
| task drift | tag changes across turns and cwd/repo/path scope |
| model behavior style | aggregate tag distributions across sessions |
| behavior diff | stack deltas between runs |

It does not analyze:

| Not supported | Why |
| --- | --- |
| correctness | no domain oracle |
| safety | tag is not a policy decision |
| validation sufficiency | requires explicit test oracle |
| claim truth | requires verifiable claim extraction and evidence oracle |

## Implementation Plan

Stage 1: Tagging only

- Parse sessions into turns, subagent calls, tool calls, and LLM calls.
- Add one-word tags for session, prompt, subagent, and LLM call.
- Validate tag format and canonicalize obvious variants.

Stage 2: Prompt-tag footprint

- Join every captured effect to its owning prompt/tool/subagent.
- Aggregate metrics by prompt tag.
- Render the Prompt-Tag Footprint Table.

Stage 3: Flamegraph stacks

- Generate folded stacks for system footprint.
- Generate folded stacks for token footprint.
- Render SVG using existing flamegraph demo infrastructure or a production renderer.

Stage 4: Diff and cross-session aggregation

- Normalize stack frames across sessions.
- Diff two runs by stack metric.
- Aggregate many sessions into behavior profiles.

Stage 5: Optional semantic cleanup

- Add repo dictionaries.
- Add tag quality reports.
- Add small-model retry only when output violates the one-word contract.

## MVP Acceptance Criteria

- 100% of accepted tags satisfy the one-word contract.
- All captured effects with complete provenance appear under exactly one prompt tag.
- Long-lived children are marked as `longlived`, not dropped.
- Coverage gaps are represented as `coverage`, not as unlinked effects.
- System and token flamegraphs use separate metrics.
- Removing tags still preserves exact system provenance; it only removes semantic grouping.

## Relationship To Existing Visualization Docs

This document refines the earlier Intent-to-Effect Flame Graph idea.

Keep:

- semantic layer + system layer in the same product;
- collapsed stacks and effect-weight widths;
- comparison across runs.

Change:

- do not use arbitrary semantic breadcrumbs as flamegraph stacks;
- use execution ownership / causal nesting as the stack grammar;
- use one-word semantic tags only as upper frames and grouping hints;
- avoid default `unlinked` language when provenance is complete.
