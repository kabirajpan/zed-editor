# AI-Native Feature Roadmap

This document outlines the vision for transforming Z3N into a state-of-the-art AI code editor, drawing from modern research and industry-leading editors like Zed and Cursor.

## 1. Core Engine Foundations (Completed)
These features provide the atomic reliability required for an AI agent to operate the editor safely.

- [x] **Atomic Transactions**: All edits (human or AI) are tracked in a transactional history, allowing for safe multi-file undos.
- [x] **Safe Selection Overwrite**: Refined coordinate shifting to ensure replacements don't leave stale highlights.
- [x] **Language-Aware Editing**: Dynamic comment tokens (`//`, `#`, `<!-- -->`) and auto-closing pairs.

## 2. Advanced AI Interaction (Proposed)
Moving beyond the chat window and into the "Editor Core."

### 👻 Ghost Speculative Diffs
*Based on Human-AI Interaction Research.*
- **Feature**: AI suggestions appear as "ghost" text in the buffer without overwriting the original.
- **Workflow**: User reviews diffs in-line and uses keyboard shortcuts to "Accept" or "Reject" specific blocks.
- **Goal**: Reduce cognitive load and give the developer back control.

### 🌳 Semantic Multibuffers
*Based on Program Comprehension Research.*
- **Feature**: Virtual views that group code blocks by *semantics* rather than file boundaries.
- **Example**: "Show me every implementation of `Trait X` across the workspace."
- **AI Use Case**: Allows the AI to perform "Project-wide refactors" while letting the user review all changes in a single vertical scroll.

### 🩹 Self-Healing Code (AI Linter)
*Based on Automatic Program Repair Research.*
- **Feature**: Direct integration with `LSP` (Language Server Protocol) errors.
- **Workflow**: When a compiler error is detected, a "Heal" button appears. The AI uses the error message and local context to propose a fix automatically.

## 3. Infrastructure for Agents
Making the editor "Operable" by LLMs.

### 🤖 Agentic Tooling API
A private API that exposes high-level actions to an LLM agent:
- `search_codebase(query)`: Vector-based semantic search.
- `apply_patch(diff)`: Safe application of complex edits.
- `run_tests()`: Feedback loop to verify AI changes.

### 🤝 CRDT-Native Collaboration
*Based on Distributed Systems Research.*
- **Goal**: Conflict-free, real-time collaboration between multiple humans and AI agents.
- **Tech**: Use `Yjs` or `Automerge` in the Rust core for peer-to-peer sync.

---

> [!IMPORTANT]
> The primary design philosophy of Z3N is **Performance First**. All AI features must be asynchronous and non-blocking to ensure the keyboard latency always remains under 10ms.
