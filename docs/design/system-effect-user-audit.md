# Agent System Effect UX Audit

日期：2026-06-13

这份文档从用户角度审视 AgentSight 的 system-effect 方向：真正要解决
什么问题，当前仓库里的 prompt/session/system 数据能看出什么、不能看出
什么，以及现有可视化和报告尝试是否有产品意义。

结论先行：

> AgentSight 不应该把自己定义成“更多 agent telemetry”。真正的问题是：
> 用户把真实系统任务委托给 agent 之后，需要一个独立、可验证、可恢复、
> 可用于调权的 system-effect receipt。

换句话说，用户不是想看更多事件，而是想回答：

```text
它改了什么？
它跑了什么？
它连了哪里？
它在哪里失败、重试或浪费？
它有没有越过我的任务边界？
这些动作分别由哪个 prompt / tool / process 导致？
如果有问题，我怎么恢复或让另一个 agent 接手？
```

## 1. 真正的用户问题

AgentSight 的核心用户问题不是 observability 本身，而是 delegation trust。

用户已经愿意让 Claude Code、Codex、Cursor、Gemini CLI 或内部 agent 做事。
但一旦 agent 能读写文件、跑命令、装依赖、访问网络、调用 MCP/server 或触碰
云资源，用户就会遇到一个新问题：

> 我没有全程盯着它，但我需要知道它对我的系统造成了什么后果。

这个问题可以拆成五个 user jobs。

### 1.1 Run Receipt

普通用户在一次 agent run 后最想知道：

```text
这次 run 到底做了什么？
```

这不是 raw log 问题。用户需要一个 receipt：

```text
Goal: fix failing parser test
Agent: claude
Duration: 13s
Model calls: 4
Tools: Read, Edit, Bash
Commands: npm test, rg ...
Files changed: src/parser.ts, tests/parser.test.ts
Network: api.anthropic.com
Tests: failed once, passed once
Unattributed: git status x6
Recovery: git revert covers tracked files; untracked generated files listed below
```

### 1.2 Recovery Context

当 agent 搞坏 repo、删文件、改配置、装依赖或产生未跟踪文件时，用户最急的
不是“审计”，而是：

```text
我需要撤回什么？
哪些东西 git diff 看不到？
哪个命令或工具导致了这个状态？
```

因此 AgentSight 的输出必须支持 recovery，不只是 accountability。

### 1.3 Permission Tuning

用户不想每一步都确认，也不想盲目开 auto mode。真正的问题是：

```text
哪些 agent 动作可以自动批准？
哪些动作必须确认？
哪些组合应该默认拒绝？
```

这需要历史 system effect，而不是 agent 自己描述的意图。例如：

```text
Safe so far:
  npm test inside workspace
  rg/read under repo

Ask:
  package manager install
  write outside workspace
  network to new domain

Deny:
  read ~/.ssh + network egress
  rm -rf outside workspace
  cloud CLI delete
```

### 1.4 PR / Code Review Due Diligence

reviewer 看到一个 AI-generated diff 时，git diff 不够。他还想知道：

```text
agent 为了这个 diff 读了哪些文件？
跑了哪些测试？
失败过几次？
有没有下载外部代码或运行安装脚本？
有没有碰 secret、CI、release、config、workspace 外路径？
```

这里 AgentSight 的价值是给 reviewer 一个 process receipt，而不是另一个
代码 diff UI。

### 1.5 Incident Forensics

出事后用户需要证据，不需要 plausible explanation：

```text
哪个 agent action 首次触碰了这个文件？
哪个进程删了它？
当时附近有什么 network call？
这个动作能否归因到某个 tool call？
```

这要求 AgentSight 明确区分 observed fact、inferred correlation 和 missing data。

## 2. 用户不关心什么

这些东西对实现重要，但不应该是默认 UX 的主语：

- syscall 列表
- raw OpenTelemetry spans
- raw HTTP bodies
- every file open
- every process child
- token table
- generic timeline
- generic process tree

它们应该作为 evidence drill-down，而不是第一屏。

默认第一屏应该是 impact summary：

```text
Impact
  Workspace: 3 intended writes, 0 deletes, 2 untracked outputs
  Commands: 4 execs, 1 failed then passed
  Network: 1 provider host, 0 unknown hosts
  Sensitive paths: none observed
  Unattributed: 6 low-risk background git probes
  Capture health: prompt observed, process observed, file writes observed, reads partial
```

## 3. 现在收集的数据是否太多太重

底层可以多收集，产品不应该多展示。

系统层事件天然会膨胀。一次 `npm test`、`cargo build`、`pip install` 或 Claude
启动流程会产生大量内部 file/process/network 行为。把这些原样展示给用户会失败。

推荐三档 capture/report 模式：

```text
Lite
  process exec/exit
  file writes/deletes/renames
  network host/path summary
  model/tool/token summary
  test/command exit summary

Normal
  Lite +
  process tree
  important file reads
  stdout/stderr summaries
  resource peaks
  attribution edges with confidence

Forensic
  Normal +
  full ordered event stream
  broad file read set
  raw request/response metadata
  payload evidence where policy allows
  low-level event replay/export
```

默认应该是 Normal 的 summary，而不是 Forensic 的 event dump。

数据存储上可以保留 raw evidence，但要有 aggressively summarized query model：

```text
raw events -> canonical events -> effect clusters -> run receipt
```

用户看到的是 effect clusters；raw events 只在追证据时展开。

## 4. 用当前 repo/session 数据做一次审计

本节基于仓库里的 demo snapshot、flamegraph prototype、当前 report/top 代码、
以及本机该 repo 的 Claude session history 做结构化分析。这里不复制完整
transcript，只引用足够支撑产品判断的摘要。

相关输入：

- `docs/design/vis/demo-data/session-20260604-221036.snapshot.json`
- `docs/design/vis/demo-data/record-smoke.snapshot.json`
- `docs/design/vis/agent_flamegraph_demo.py`
- 本地 Claude session：
  `/home/yunwei37/.claude/projects/-home-yunwei37-workspace-agentsight/5db7e4bd-3d95-46c0-8827-40cef1cf9f44.jsonl`

### 4.1 从 prompt/session history 能看出什么

以一个 Claude run 为例，session history 能清楚看出：

- user goal：实现 B-tree、测试、复杂度分析
- semantic plan：实现数据结构、写测试、写复杂度文档
- tool intent：
  - `Write /home/yunwei37/workspace/agentsight/btree.py`
  - `Write /home/yunwei37/workspace/agentsight/test_btree.py`
  - `Write /home/yunwei37/workspace/agentsight/btree_complexity_analysis.md`
  - `Bash python3 test_btree.py`
- claimed/observed tool result：测试输出显示 28 tests passed
- final assistant claim：交付了 3 个文件，测试通过

这说明 prompt/session history 很适合提供 semantic layer：

```text
用户想要什么
agent 计划怎么做
agent 选择了什么工具
工具返回了什么结果
agent 最后声称完成了什么
```

没有这层，system events 无法回答“为什么”。

### 4.2 从 system effect 数据能看出什么

demo snapshot 里的 system 层能看出另一类事实：

- process root：Claude 以 Node 进程运行
- startup/process 行为：`git`、`sh`、`which`、`bash`、`locale`、`rg`、`ldd` 等
- network：
  - `api.anthropic.com /v1/messages?beta=true`
  - Anthropic CLI settings/client_data endpoints
  - `raw.githubusercontent.com` changelog request
- file writes：
  - `~/.claude.json.backup`
  - `~/.claude.json.tmp...`
  - `~/.claude/projects/...jsonl`
  - `~/.claude/statsig/...`
  - `~/.claude/shell-snapshots/...`
  - workspace temp writes such as `btree.py.tmp...`
  - Claude plugin marketplace `.git` files under `~/.claude/plugins/...`
- notable behavior：Claude startup cloned or updated an official plugin marketplace under
  `~/.claude/plugins/marketplaces/...`

这说明 system layer 很适合提供 reality check：

```text
真实进程树
真实文件写入
真实网络目的地
真实退出码
真实资源消耗
agent transcript 没显式提到的背景行为
```

没有这层，agent 的 summary 只是 self-report。

### 4.3 当前 export/snapshot 能看出的东西

当前 exported snapshot 字段已经有可用骨架：

```text
sessions
token_summary
tool_calls
process_nodes
audit_events
network_targets
resource_samples
summary
```

对 prototype 来说，这已经足够生成：

- process-time flamegraph
- intent-effect flamegraph prototype
- retry/failure view
- impact-zones view

这证明仓库里的尝试不是空想：现有数据已经能支撑第一版静态 SVG 和 summary。

### 4.4 当前数据不能可靠看出的东西

真正产品化时，这些缺口比画图样式更重要。

第一，run boundary 不干净。

demo snapshot 混入了多个 agent-native sessions 和一个 recorded smoke run 的
process evidence。它适合 stress-test renderer，但不适合直接作为用户的
“一次 run receipt”。用户需要的是：

```text
这个 run 的 root process 是谁？
哪些 prompt/session 属于它？
哪些 process/file/network events 属于它？
哪些只是历史记录或全局背景？
```

第二，tool payload 在 exported `tool_calls` 里太 lossy。

snapshot 里能看到大量 `Bash`、`Read`、`Edit`、`exec_command`、`write_stdin`
计数，但 many rows 的 `input` / `output` 是 `{}`。而原始 Claude JSONL 里
确实有 file path、command 和 tool output。产品上不能只保留 tool name；
否则 `Bash` 这个词无法变成 effect attribution。

第三，final filesystem state 不明确。

system event 捕到了 `btree.py.tmp...` 这类临时写入，但最终文件当前不在
workspace 中。用户真正关心的是：

```text
最终留下了什么？
哪些 tracked files changed？
哪些 untracked files were created？
哪些 temp writes were renamed, deleted, or abandoned？
git diff 能覆盖哪些？
```

仅有 write event 不足以回答 recovery。

第四，prompt/session history 的成功声明不能替代系统验证。

Claude transcript 里 `Bash python3 test_btree.py` 的 tool result 显示 28 tests
passed。但对应 demo snapshot 的 process_nodes 中没有清晰的 `python3` test
process。也就是说，在当前 artifact 里，测试通过主要来自 tool transcript，
不是独立 system observation。

这不一定说明 tracer 做不到，而是说明当前 demo artifact 还不能对用户说：

```text
系统层独立验证了测试进程执行并 exit 0。
```

第五，read coverage 和 sensitive access 证明不足。

用户会问“有没有读 `.env`、SSH key、cloud credentials、browser profile”。
如果默认数据主要展示 writes，或者 file reads 被过滤/折叠，就不能给出强
negative claim。报告应该说：

```text
Sensitive reads: none observed
Coverage: broad file-read capture disabled / filtered
```

不要把 “not observed” 写成 “did not happen”。

第六，network effect 还缺 semantic classification。

`api.anthropic.com` 是 provider traffic，`raw.githubusercontent.com` 可能是
CLI update/changelog check，package registries 可能是 dependency activity。
用户不想只看 host list，而想知道：

```text
这是 provider 调用、tool 调用、dependency download、unknown egress，还是 cloud mutation？
```

第七，unattributed 不等于 suspicious。

很多 background activity 是 agent runtime 启动行为，例如 shell snapshot、
git probes、plugin marketplace update、telemetry/log writes。它们应该可见，
但不要默认渲染成安全告警。更好的标签是：

```text
unattributed / runtime background / expected startup / unknown
```

## 5. 仓库里的尝试是否有意义

有意义，但需要重新排序产品优先级。

### 5.1 有意义的部分

#### Product scope 文档方向正确

`docs/design/product-scope-agent-native.md` 把问题定义为 accountability for
delegation，而不是“agent 自动化本身”。这是正确的产品边界：

```text
agent 做任务
AgentSight 证明发生了什么、保存证据、帮助调权和恢复
```

这应该保留为总原则。

#### Visualization docs 的核心判断正确

`docs/design/vis/README.md`、`agent-run-map.md`、`run-impact-map.md`、
`intent-to-effect-flame-graph.md` 的共同判断是对的：

```text
图的中心不是 prompt，也不是 process，而是 intent -> effect edge。
```

这是 AgentSight 与 LangSmith/Langfuse/Phoenix 和 strace/eBPF dashboard 的
差异点。

#### Flamegraph prototype 有证明价值

`docs/design/vis/agent_flamegraph_demo.py` 证明：

- exported snapshot 已经能生成静态 SVG
- folded stack 形态适合 agent intent/effect 层级
- impact-zones、retry-failure、process-time 等投影可以共享同一份数据

这对设计探索很有价值。

#### `top` / `stat` / `report` 的系统工具隐喻是对的

借 `perf stat`、`perf top`、`perf record`、`perf report` 的交互模型很合适。
AgentSight 是 local systems tool for agent runs，不应该变成纯 SaaS dashboard。

#### agent-native session importer 是必要 bridge

从 Claude/Codex 本地 session history 提取 prompt、tool、token、cwd、model 是
很务实的路线。很多用户并不会主动给应用加 SDK instrumentation。本地 agent
history 是现成 semantic layer。

### 5.2 目前不够产品化的部分

#### 当前第一屏还太像 telemetry

timeline、process tree、metrics、raw audit 都是有用 drill-down，但不是用户的
第一需求。第一屏应该是 run receipt / impact summary。

#### Flamegraph 现在不应该成为唯一主界面

Intent-to-Effect Flame Graph 很适合回答“哪里最重、最贵、最有影响”，也适合
做静态 artifact。但它不是最适合回答：

```text
这次 run 是否安全？
我该怎么恢复？
哪些动作能自动批准？
```

所以 flamegraph 应该是 report 的一个 section 或 export artifact，而不是
取代 Run Receipt。

#### `effect weight` 需要用户语义校准

当前 prototype 的 mixed score 有方向，但用户不会天然理解：

```text
duration + process count + files + network + failed exits + semantic signals
```

更好的产品做法是同时给出多个排序：

```text
Most changed
Most risky
Most expensive
Most repeated
Most unattributed
```

而不是让一个综合分承担所有解释。

#### 需要更强 capture health

用户必须知道报告能证明什么，不能证明什么。每份 report 都应该有：

```text
Capture health
  Prompt/tool history: observed from Claude JSONL
  LLM network: observed via TLS / not observed
  Process exec/exit: observed
  File writes: observed
  File reads: partial / disabled / filtered
  Network hosts: observed
  Payloads: redacted / disabled / unavailable
  Attribution: direct / PID lineage / timestamp inferred
```

没有 capture health，报告容易过度承诺。

## 6. 从用户角度的理想默认报告

默认 `agentsight report` 应该输出类似下面的结构。

```text
AgentSight Report

Run
  Agent: claude
  Goal: Write B-tree implementation, tests, and complexity analysis
  Duration: 3m04s
  Boundary: root pid 881192, cwd /home/yunwei37/workspace/agentsight

Receipt
  Intended outputs:
    btree.py
    test_btree.py
    btree_complexity_analysis.md

  Observed workspace effects:
    wrote temp content for btree.py
    wrote temp content for test_btree.py
    wrote temp content for btree_complexity_analysis.md
    final file state: not present in workspace at report time

  Commands:
    transcript: Bash("python3 test_btree.py") -> 28 tests passed
    system process evidence: not matched in this snapshot

  Network:
    provider: api.anthropic.com
    runtime/check: raw.githubusercontent.com
    unknown egress: none observed

  Runtime/background:
    wrote ~/.claude config/log/session files
    updated/cloned Claude plugin marketplace under ~/.claude/plugins
    ran git/shell startup probes

  Risk:
    workspace writes: intended
    home-directory writes: runtime background
    external network: provider + runtime check
    sensitive path reads: cannot make strong claim; read coverage partial

  Attribution:
    transcript -> tool intent: direct
    process/file -> root agent process: observed
    tool intent -> individual OS effect: partial/inferred

  Recovery:
    check git status for tracked files
    inspect/remove listed untracked/temp files
    inspect ~/.claude runtime changes only if investigating agent runtime behavior
```

这个报告比 raw timeline 更有用，因为它直接支持用户决策：

- 接受结果
- rerun/verify
- revert
- tighten permissions
- investigate background activity

## 7. 推荐的数据模型

现在的数据表可以继续保留，但需要在 query layer 加一个用户中心模型。

```text
Run
  id
  root command / pid / session path
  start/end
  workspace
  capture health

Intent
  user goal
  LLM turn
  plan step
  tool call
  claimed result

Effect
  process exec/exit
  file read/write/delete/rename/create
  network host/path/method/status
  resource sample
  generated artifact
  test result

AttributionEdge
  from intent
  to effect
  evidence kind:
    direct tool payload
    pid lineage
    session log path
    cwd/time window
    stdout/stderr correlation
  confidence

EffectCluster
  user-facing summary
  evidence pointers
  risk label
  recovery hint
```

`EffectCluster` 是 UX 的主对象。raw event 不是。

## 8. 报告应该优先展示的用户指标

建议把默认 summary 做成这些指标，而不是 telemetry count dump。

### Impact Radius

```text
workspace writes
workspace deletes
workspace external writes
network destinations
process families
generated artifacts
cloud/API mutation candidates
```

### Attribution Coverage

```text
effects directly attributed to tool calls
effects attributed only to root process
unattributed effects
background/runtime effects
```

### Recovery Confidence

```text
high: tracked file changes only, no external writes
medium: untracked files or dependency/cache writes
low: deletes, external workspace writes, cloud/network mutations
unknown: capture coverage insufficient
```

### Autonomy Signal

```text
auto-approvable patterns
needs-confirmation patterns
deny candidates
new behavior compared with prior runs
```

### Waste / Loop Signal

```text
repeated commands
repeated file reads
failed exits
large token spend with no new system effect
long idle windows
```

## 9. Product Priority

建议按这个顺序推进。

### P0: Clean Run Receipt

先做一个可信的 `agentsight report`，不要先追求复杂图。

必须具备：

- clean run boundary
- prompt/tool summary
- commands/processes
- files changed/created/deleted
- network destinations
- unattributed/background section
- capture health
- evidence pointers

### P1: Attribution Edges

把每个 effect 尽量连回 tool intent，并明确 confidence。

边类型至少包括：

- direct tool payload -> known file path / command
- pid lineage -> process/file/network
- session log path -> agent process
- timestamp/cwd inferred -> weak attribution
- unmatched -> background/unattributed

### P2: Recovery / PR Review Reports

在 receipt 上扩展两个强场景：

```bash
agentsight report --recovery
agentsight report --for-pr
```

这比单独做 dashboard 更容易产生用户价值。

### P3: Flamegraph and Impact Maps

等 receipt model 稳定后，把 flamegraph 作为 report/export artifact：

```bash
agentsight report --flamegraph out.svg
agentsight report --impact-map out.html
```

这时图才不会只是漂亮的 telemetry visualization，而是 receipt 的视觉投影。

### P4: Policy Suggest / Airlock

只有当 receipt 和 attribution 足够可靠后，再做 permission tuning 和 live blocking。
否则 policy 会建立在不稳定证据上。

## 10. 对当前仓库的具体建议

### 10.1 保留并收敛设计文档

保留：

- `product-scope-agent-native.md`
- `docs/design/vis/README.md`
- `agent-run-map.md`
- `run-impact-map.md`
- `intent-to-effect-flame-graph.md`
- `agent-workspace-map.md`

但需要在 README 或 product scope 里明确：

```text
Default product surface = Run Receipt
Flamegraph = one drill-down/export view
Timeline/process tree = forensic drill-down
```

### 10.2 改进 exported snapshot

`tool_calls` 应该保留 sanitized input/output summaries，而不是只有 tool name。

最小字段：

```text
tool_name
tool_call_id
start/end
input_summary
output_summary
file_path
command
cwd
status
is_error
redaction_status
```

### 10.3 加 RunReceipt query model

不要让 CLI report 直接从 raw rows 拼字符串。增加 typed query model：

```text
collector/src/report/
  model.rs
  receipt.rs
  attribution.rs
  risk.rs
  render_text.rs
  render_json.rs
```

### 10.4 把 capture health 做成一等字段

每个 report/export 都应该告诉用户：

```text
what was observed
what was inferred
what was not captured
```

这是建立信任的关键。

### 10.5 先做低噪声 summary，再做 UI

Web UI 现在的 timeline/process tree 可以保留，但第一屏应该改成：

```text
Run Receipt
Impact Radius
Intent -> Effect Chain
Unattributed Activity
Recovery / Review Notes
Evidence Drill-down
```

### 10.6 给 background 建分类

不要只有 unattributed。至少分：

```text
agent runtime background
shell startup
package manager internal
VCS metadata
workspace task
unknown background
```

这能显著减少误报感。

### 10.7 不要对 “not observed” 过度承诺

报告文案必须避免：

```text
No secrets were read.
```

除非 read coverage 足够强。更安全的表达是：

```text
No sensitive reads observed.
File-read capture: partial.
```

## 11. 最终判断

当前仓库里的尝试有意义，尤其是 intent-to-effect 可视化和 agent-native session
import。这些都指向一个真实空白：应用层 agent trace 看不到系统后果，系统层
eBPF trace 看不懂 agent 意图。

但产品上不要先卖“火焰图”或“timeline”。用户真正要的是：

```text
一份可信的 agent run receipt：
  证明 agent 做了什么
  说明它为什么这么做
  显示实际系统影响
  标出无法归因和无法证明的部分
  帮助用户接受、回滚、调权或继续调查
```

AgentSight 的差异化不是收集最多事件，而是把 prompt/session history 和
system effect 连接成用户可决策的证据。

