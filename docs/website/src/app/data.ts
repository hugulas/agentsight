// SPDX-License-Identifier: MIT
// Copyright (c) 2026 eunomia-bpf org.

import {
  Activity,
  BadgeCheck,
  Boxes,
  FileSearch,
  Gauge,
  GitPullRequest,
  LockKeyhole,
  Network,
  ScanLine,
  ShieldCheck,
  SquareTerminal,
  TimerReset,
  Workflow,
  Zap,
} from 'lucide-react';

export const navItems = [
  { href: '/use-cases/', label: 'Use cases' },
  { href: '/compare/', label: 'Compare' },
  { href: '/security/', label: 'Security' },
  { href: 'https://eunomia.dev/agentsight/', label: 'Docs' },
];

export const signalPillars = [
  { label: 'Time', value: 'LLM, shell, and wait time', icon: TimerReset },
  { label: 'Tokens', value: 'Turns, models, and loops', icon: Gauge },
  { label: 'Commands', value: 'Exec tree and exit status', icon: SquareTerminal },
  { label: 'Files', value: 'Reads, writes, deletes', icon: FileSearch },
  { label: 'Network', value: 'Endpoints and processes', icon: Network },
  { label: 'Resources', value: 'CPU, memory, sessions', icon: Activity },
];

export const useCases = [
  {
    title: 'Profile slow or expensive agent runs',
    description:
      'Break down a run by LLM time, shell time, repeated file scans, network waits, token loops, CPU, and memory.',
    icon: Gauge,
    href: '/use-cases/#slow-runs',
  },
  {
    title: 'Review AI-generated PRs faster',
    description:
      'Attach a run profile with commands, tests, retries, files touched, network calls, and resource cost.',
    icon: GitPullRequest,
    href: '/use-cases/#pr-review',
  },
  {
    title: 'Trace closed-source agent CLIs',
    description:
      'Monitor Claude Code, Codex, Gemini CLI, OpenCode, OpenClaw, and local commands without vendor hooks.',
    icon: ScanLine,
    href: '/use-cases/#closed-source',
  },
  {
    title: 'Generate shareable insight pages',
    description:
      'Use AgentSight skills to turn local traces into executive summaries, findings, tables, and single-file HTML reports.',
    icon: Workflow,
    href: '/use-cases/#insight-artifacts',
  },
  {
    title: 'Audit MCP servers, skills, and plugins',
    description:
      'Run tools under system-level tracing to see file writes, child processes, external calls, and token impact.',
    icon: Boxes,
    href: '/use-cases/#tool-audit',
  },
];

export const comparisonRows = [
  {
    feature: 'Integration model',
    sdk: 'SDK, callback, wrapper, or auto-instrumentation',
    gateway: 'Route provider traffic through a proxy',
    transcript: 'Read agent logs after the fact',
    agentsight: 'Attach from the OS boundary with eBPF',
  },
  {
    feature: 'Closed-source CLIs',
    sdk: 'Limited to exposed hooks',
    gateway: 'Only sees routed LLM traffic',
    transcript: 'Depends on saved local format',
    agentsight: 'Traces the process and child processes directly',
  },
  {
    feature: 'Commands and subprocesses',
    sdk: 'Only when the app reports them',
    gateway: 'Not visible',
    transcript: 'Often summarized or incomplete',
    agentsight: 'Process tree, argv, status, and timing',
  },
  {
    feature: 'File and network effects',
    sdk: 'Framework-specific and often missing',
    gateway: 'Provider traffic only',
    transcript: 'Only what the agent logs',
    agentsight: 'Reads, writes, deletes, endpoints, and attribution',
  },
  {
    feature: 'Best fit',
    sdk: 'Your own LLM app',
    gateway: 'Provider routing and cost controls',
    transcript: 'Cheap historical usage reports',
    agentsight: 'System-level run profiling and incident analysis',
  },
];

export const securityPoints = [
  {
    title: 'Local-first data path',
    description:
      'Recorded sessions are saved locally unless you explicitly export or serve them. Captured data can include prompts, responses, paths, headers, and network targets.',
    icon: LockKeyhole,
  },
  {
    title: 'Targeted tracing',
    description:
      'AgentSight is designed around recording the selected agent command, process, or session rather than turning the product site into a hosted collector.',
    icon: ShieldCheck,
  },
  {
    title: 'No SDK, no proxy, no vendor hooks',
    description:
      'The profiler observes stable system boundaries, so it can work across runtimes and closed-source CLIs where application traces stop.',
    icon: BadgeCheck,
  },
  {
    title: 'OTel-compatible output',
    description:
      'Captured LLM calls can be exported as OpenTelemetry GenAI spans for teams that already operate standard telemetry pipelines.',
    icon: Workflow,
  },
];

export const heroStats = [
  { label: 'Overhead target', value: '~3%' },
  { label: 'Integration', value: 'Zero SDK' },
  { label: 'Scope', value: 'Process + LLM + files + network' },
];

export const quickstart = [
  'cargo install agentsight',
  'sudo agentsight record -- claude',
  'agentsight report export -o snapshot.json',
];

export const artifactFlow = [
  {
    title: 'Capture the run',
    description: 'Record the agent session, including model calls, process tree, files, network, and resource samples.',
  },
  {
    title: 'Let a skill analyze it',
    description: 'An AgentSight skill reads the saved DB or snapshot and turns raw events into ranked findings and evidence tables.',
  },
  {
    title: 'Share the artifact',
    description: 'Export a self-contained HTML report for PR review, tool evaluation, incident notes, or team retrospectives.',
  },
];

export const trustSignals = [
  { label: 'Open source', value: 'MIT licensed', icon: BadgeCheck },
  { label: 'Research-backed', value: 'arXiv + ACM DOI', icon: FileSearch },
  { label: 'Built by Eunomia', value: 'eBPF systems team', icon: Zap },
];
