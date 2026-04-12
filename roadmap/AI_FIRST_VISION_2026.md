# AI-First Code Editor — Feature Roadmap

> Built from scratch. Every feature designed for AI collaboration, not retrofitted onto it.

---

## Core Architecture (Already Built)

| Component | Details |
|---|---|
| **Rope Data Structure** | Logarithmic complexity edits, no full string copying, efficient for large files |
| **Tree-sitter Integration** | Incremental parsing, only re-parses changed parts, real-time AST |
| **Virtual Scrolling** | Viewport-based rendering, only visible lines rendered |
| **Syntax Highlighting** | AST-driven, accurate, real-time |
| **Auto Indentation** | Formatting support during editing |
| **Rendering Pipeline** | Separation between data layer (rope) and view layer |

---

## Layer 1 — AI-Native Core Primitives
> Features only possible because we own the rope + AST from scratch

### 1.1 Semantic Cursor
Instead of "cursor is at line 34, col 12":
```
cursor is inside → function parseForm()
                 → parameter 2
                 → type: str
                 → called from → forms.py:validate()
```
- Tree-sitter provides this context natively
- AI receives semantic position, not just byte offset
- Enables smarter, location-aware suggestions
- **Source:** Architectural advantage — no competitor has this

---

### 1.2 Structured Edit History with AI Authorship
- Every edit tagged: `human` | `ai_suggested` | `ai_accepted` | `ai_modified`
- "Undo AI suggestion" as a first-class operation
- Edit trajectory stored in rope's mutation log — not scraped from text buffer
- Enables training data collection from real usage
- **Source:** Architectural advantage — edit history is a native data structure

---

### 1.3 Streaming Tokens → Rope Directly
- Model streams tokens → land directly into rope incrementally
- Tree-sitter re-parses as tokens arrive
- No "ghost text" hack like Copilot
- AST stays valid during streaming
- **Source:** Architectural advantage — competitors patch text after the fact

---

### 1.4 Incremental KV Cache Updates (PIE)
- When developer edits code, only feed the delta to the model — not the full file
- Based on: **Positional Integrity Encoding (PIE)** — arXiv:2407.03157
- Rope diff → AST diff → KV cache patch
- Significant latency reduction on every keystroke
- **Source:** *"Let the Code LLM Edit Itself When You Edit the Code"* — arXiv:2407.03157

---

## Layer 2 — Proactive AI Intelligence
> AI that acts without being asked

### 2.1 Next Edit Prediction (NEP)
- Watches your edit sequence: rename → extract → refactor
- Predicts **both location and content** of your next edit
- No natural language instruction required
- Surfaces suggestion via Tab key — zero interruption to flow
- Benchmark: **75.6% location accuracy, 43.44% edit acceptance rate** at Ant Group (20,000 devs)
- Delivery in **under 250ms**
- **Source:** *"Next Edit Prediction"* — arXiv:2508.10074; *"NES"* — arXiv:2508.02473; *"CoEdPilot"* — ISSTA '24

---

### 2.2 Intent Modeling from Edit Trajectory
- Builds a running hypothesis of what the developer is trying to do
- Based on: what you rename, extract, undo, accept, reject
- When confidence crosses threshold → proposes next action without prompting
- Zero-prompt collaboration paradigm
- Shifts from reactive (answer questions) → proactive (anticipate goals)
- **Source:** *"Towards Decoding Developer Cognition"* — arXiv:2501.02684; *"Prompting LLMs for Code Editing"* — arXiv:2504.20196

---

### 2.3 Suggestion Confidence Gating
- Suggestions only surface when acceptance probability is predicted high
- Eliminates the **70% rejection rate** problem found in GitHub Copilot research
- Per-suggestion feedback loop: accept / reject / modify stored in edit history
- Personalizes over time per developer's style
- **Source:** *"Predicting Developer Acceptance of AI-Generated Code Suggestions"* — arXiv:2601.21379

---

## Layer 3 — Repository Intelligence
> Understanding the whole codebase, not just the open file

### 3.1 Live Knowledge Graph (AST-driven)
- Built on Tree-sitter — graph is **always live**, not batch-indexed
- Every edit → rope delta → AST diff → graph update
- Captures: function calls, type hierarchies, module dependencies, naming conventions
- Enables: cross-file reasoning, global semantic consistency
- vs. Competitors: they re-index periodically as text snapshots (like Google crawling a webpage)
- **Source:** *"Knowledge Graph Based Repository-Level Code Generation"* — arXiv:2505.14394; *"RACG Survey"* — arXiv:2510.04905

---

### 3.2 Repository-Level RAG (RACG)
- Chunks code by function/class using AST (not naive line-based chunking)
- Dual index: vector search (ChromaDB/similar) + graph-based queries
- Retrieves relevant context across the entire repo — not just open files
- Feeds AI accurate, structured context instead of raw text dumps
- **Source:** *"RACG Survey"* — arXiv:2510.04905; *"Context Engineering for Multi-Agent LLM Code Assistants"* — arXiv:2508.08322

---

### 3.3 AST-Level Edit Planning (Plan Mode)
- Before executing any change, show a structured plan:
  - Which functions change
  - In what order
  - What the call graph impact is
- Plan is AST-level, not line-level → meaningful and verifiable
- Developer reviews plan → approves → agent executes
- **Source:** *"A Survey on Code Generation with LLM-based Agents"* — arXiv:2508.00083

---

## Layer 4 — Persistent AI Memory
> AI that remembers across sessions

### 4.1 MemPalace-Style Editor Memory
Inspired by MemPalace (Milla Jovovich + Ben Sigman, April 2026 — 34,000 GitHub stars in 72hrs)

**Core insight:** Don't summarize. Store everything. Make it findable.
- Existing tools (Mem0, Zep) let AI decide what to remember → lossy
- MemPalace stores verbatim → searches with embeddings → **96.6% recall**

**Mapped to your editor:**

| MemPalace Concept | Editor Equivalent |
|---|---|
| Wings (projects/people) | Repositories / modules |
| Halls (memory types) | Decisions / bugs / patterns / todos |
| Rooms (topics) | Functions / features |
| Closets (compressed chunks) | Edit history segments |
| Tunnels (cross-references) | AST call graph edges |

**Hierarchical search boost:**
```
Flat search:          60.9% accuracy
+ Wing scope:         73.1% (+12%)
+ Hall scope:         84.8% (+24%)
+ Room scope:         94.8% (+34%)
```

**Your advantage over MemPalace:**
MemPalace infers structure from chat logs with regex patterns.
Your editor *already has* the structure from Tree-sitter — functions, modules, call graphs are live facts, not inferred patterns.

**Source:** MemPalace — github.com/milla-jovovich/mempalace; *"Raw Text Beats Extracted Memory"* (arXiv draft)

---

### 4.2 Session Memory Layers
```
L0 (~50 tokens)   → always loaded: active repo, current file, developer preferences
L1 (~120 tokens)  → always loaded: recent decisions, known patterns in this module
L2+               → on-demand: search palace when deeper context needed
```
- AI "wakes up" informed in 170 tokens
- Searches only when needed — cheap and fast
- No full context dump on every request

---

### 4.3 Temporal Entity Graph
- SQLite-backed graph of: what changed, when, why (from commit messages + edit history)
- Enables queries like: "What was this function doing 2 weeks ago?"
- "When did we introduce this dependency?"
- Time-aware retrieval — not just semantic similarity

---

## Layer 5 — Competitive Differentiators
> Things nobody else ships

### 5.1 Multi-Turn Program Synthesis
- AI works in structured multi-turn conversation tied to the AST
- Each turn refines a plan node, not a free-form text exchange
- Factorizes complex tasks into verifiable subtasks
- **Source:** *"CodeGen: Multi-Turn Program Synthesis"* — arXiv:2203.13474

---

### 5.2 Real-Time Collaboration (CRDT-based)
- Like Zed — multiple developers + AI agents editing simultaneously
- CRDT (Conflict-free Replicated Data Types) — same tech as Google Docs
- AI agent edits and human edits merge without conflict
- AI is a participant in the session, not a sidebar chatbot
- **Source:** Zed architecture research

---

### 5.3 Agent Client Protocol (ACP) Support
- Open protocol (pioneered by Zed + JetBrains in Jan 2026)
- External agents (Claude Code, Codex, etc.) plug into your editor natively
- Editor becomes an "agent control plane" not just a text editor
- **Source:** Zed ACP announcement, Jan 2026

---

### 5.4 MCP (Model Context Protocol) Integration
- Connect editor to external tools: databases, GitHub, APIs, filesystems
- AI agent can query your Oracle EPM instance, your DB, your docs — directly
- Tool calls happen inside the editor's permission system
- **Source:** Industry standard — Zed, Cursor, VS Code all ship this

---

## Layer 6 — Developer Experience
> Features that make the editor feel alive

### 6.1 AI Edit Authorship Visualization
- Inline diff view: show which lines were AI-written vs human-written
- Color-coded by confidence level of AI suggestion
- "Provenance mode" — see the history of every line

---

### 6.2 Cognitive Load Reduction
- AI detects when developer is in "flow" vs "stuck"
- Suppresses suggestions when in flow (high edit velocity)
- Surfaces help proactively when stuck (long pause + repeated undos)
- **Source:** *"Towards Decoding Developer Cognition"* — arXiv:2501.02684

---

### 6.3 Autonomy Slider
- Per-session control: how much the AI does on its own
  - `Tab completion` → `Inline edits` → `File-level agent` → `Repo-level agent`
- Explicit permission model — every tool call requires approval or auto-approval
- No "YOLO mode" security foot-gun like VS Code ships

---

### 6.4 Multibuffer View
- Compose excerpts from across the entire codebase into one editable surface
- See function definition + all callers + all tests — in one view
- Edits propagate back to source files
- **Source:** Zed multibuffers feature

---

## What No Competitor Has

| Feature | VS Code | Cursor | Zed | Z3N | **Ours** |
|---|---|---|---|---|---|
| Live AST as AI context | ❌ | ❌ | Partial | ❌ | ✅ |
| Semantic cursor | ❌ | ❌ | ❌ | ❌ | ✅ |
| Rope edit log as intent signal | ❌ | ❌ | ❌ | ❌ | ✅ |
| Incremental KV cache (PIE) | ❌ | ❌ | ❌ | ❌ | ✅ |
| Live knowledge graph | ❌ | ❌ | ❌ | ❌ | ✅ |
| AST-chunked RAG | ❌ | ❌ | ❌ | ❌ | ✅ |
| Streaming tokens → rope | ❌ | ❌ | ❌ | ❌ | ✅ |
| MemPalace-style code memory | ❌ | ❌ | ❌ | ❌ | ✅ |
| AI authorship in edit history | ❌ | ❌ | ❌ | ❌ | ✅ |
| Next Edit Prediction (NEP) | ❌ | Partial | ❌ | ❌ | ✅ |

---

## Research Papers Referenced

| Paper | arXiv | Key Feature |
|---|---|---|
| Next Edit Prediction | 2508.10074 | NEP — proactive edit suggestions |
| NES: Next Edit Suggestion | 2508.02473 | Instruction-free, 250ms, 75.6% accuracy |
| CoEdPilot | ISSTA '24 | Edit propagation across files |
| RACG Survey | 2510.04905 | Repo-level RAG for code |
| Knowledge Graph Repo Code Gen | 2505.14394 | Live KG for codebase context |
| Context Engineering Multi-Agent | 2508.08322 | AST-chunked retrieval pipeline |
| Let Code LLM Edit Itself (PIE) | 2407.03157 | Incremental KV cache updates |
| Predicting Developer Acceptance | 2601.21379 | Confidence gating for suggestions |
| Decoding Developer Cognition | 2501.02684 | Cognitive load + intent modeling |
| Prompting LLMs for Code Editing | 2504.20196 | Intent communication struggles |
| LLM Agent Code Generation Survey | 2508.00083 | Structured planning before codegen |
| CodeGen Multi-Turn Synthesis | 2203.13474 | Multi-turn structured synthesis |
| MemPalace (Jovovich + Sigman) | github.com/milla-jovovich/mempalace | Verbatim memory + hierarchical retrieval |

---

## Industry Research Referenced

| Source | Key Insight |
|---|---|
| Zed (2026) | CRDT collaboration, ACP protocol, Zeta2 intent model |
| Cursor v3.0 (2026) | Background agents, BugBot self-improvement |
| Google Z3N (2026) | Manager View, parallel agents, browser subagents |
| VS Code 1.110 (2026) | Agent memory across sessions, context compaction |
| GitHub Copilot Study | 70% suggestion rejection rate — gating needed |
| Ant Group NES Deployment | 51.55% location acceptance, 43.44% edit acceptance |

---

*Last updated: April 2026*
*Status: Living document — add features as research continues*
