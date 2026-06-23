// SPDX-License-Identifier: MIT
// Copyright (c) 2026 eunomia-bpf org.

import type { Metadata, Viewport } from 'next';
import './globals.css';

export const metadata: Metadata = {
  metadataBase: new URL('https://agentsight.us'),
  title: {
    default: 'AgentSight by Eunomia — System-Level AI Agent Profiling with eBPF',
    template: '%s | AgentSight by Eunomia',
  },
  description:
    'Profile AI agent runs across time, tokens, commands, files, network calls, and system resources. Zero-SDK eBPF tracing for Claude Code, Codex, Gemini CLI, and local agents.',
  alternates: {
    canonical: '/',
  },
  openGraph: {
    title: 'AgentSight by Eunomia — System-Level AI Agent Profiling with eBPF',
    description:
      'Profile AI agent runs across time, tokens, commands, files, network calls, and system resources. No SDKs, proxies, or vendor hooks.',
    url: 'https://agentsight.us/',
    siteName: 'AgentSight',
    type: 'website',
    images: ['/images/top-mode-demo.png'],
  },
  twitter: {
    card: 'summary_large_image',
    title: 'AgentSight by Eunomia',
    description: 'System-level profiling and tracing for AI agents, powered by eBPF.',
    images: ['/images/top-mode-demo.png'],
  },
};

export const viewport: Viewport = {
  width: 'device-width',
  initialScale: 1,
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
