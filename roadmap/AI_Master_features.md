# AI-First Code Editor — Master Feature Document

> **Philosophy:** Every feature is designed for AI collaboration from the ground up.  
> Not a traditional editor with AI bolted on. An AI system that is also an editor.

---

## Table of Contents

1. [Core Architecture (Built)](#1-core-architecture)
2. [AI-Native Primitives](#2-ai-native-primitives)
3. [Proactive Intelligence](#3-proactive-intelligence)
4. [Formal Verification Layer](#4-formal-verification-layer)
5. [Repository Intelligence](#5-repository-intelligence)
6. [Persistent Memory System](#6-persistent-memory-system)
7. [Computational Engineering Model (Noyron Approach)](#7-computational-engineering-model)
8. [Collaboration & Agent Protocol](#8-collaboration--agent-protocol)
9. [Developer Experience](#9-developer-experience)
10. [Competitive Comparison](#10-competitive-comparison)
11. [Research References](#11-research-references)

---

## 1. Core Architecture

> Already built. The foundation everything else sits on.

### 1.1 Rope Data Structure
**What:** Text is stored as a balanced binary tree of string chunks instead of a flat string.  
**Why it matters:**
- Insertion and deletion are `O(log n)` — never copies the full file
- Efficiently handles files of any size (100k+ lines with no lag)
- Mutation log is a native data structure — records every edit as a timestamped event
- This log becomes the input for AI intent modeling (see §3.2)

**Competitors:** VS Code uses a piece tree. Cursor inherits it. Neither was designed as an AI collaboration primitive.

---

### 1.2 Tree-sitter — Incremental Syntax Engine
**What:** An incremental parsing system that maintains a live, accurate AST.  
**Why it matters:**
- Only re-parses the *changed portion* of the file — not the entire file on every keystroke
- AST is always valid, always current, never stale
- Supports 100+ languages with the same interface
- The AST is the foundation for: syntax highlighting, semantic cursor, knowledge graph, RAG chunking, Z3 verification, and edit planning

**Key insight:** Competitors use Tree-sitter as a *highlighting plugin*. We use it as a *core data structure* that the entire AI layer reads from.

---

### 1.3 Virtual Scrolling / Viewport Rendering
**What:** Only the visible portion of the file is rendered to the DOM/UI at any time.  
**Why it matters:**
- Files of any size open instantly
- Smooth scrolling regardless of file length
- Rendering cost stays constant — never scales with file size

---

### 1.4 Rendering Pipeline
**What:** Clean separation between data layer (rope) and view layer.  
**Layers:**
```
Rendering Layer
    ↓
Viewport / Virtual Scrolling
    ↓
Text Engine (Rope)
    ↓
Syntax Engine (Tree-sitter AST)
    ↓
Editing Operations (CRUD, Clipboard, Formatting)
```

---

## 2. AI-Native Primitives

> Features that are only possible because we own the rope + AST from scratch.  
> Impossible to implement cleanly in a retrofitted editor.

### 2.1 Semantic Cursor
**What:** The cursor position is expressed in AST terms, not just line/column.  
**How it works:**
```
Traditional:   "cursor is at line 34, col 12"

Ours:          cursor is inside → function parseForm()
                                → parameter index: 2
                                → type: str
                                → called from → forms.py:validate()
                                → last modified: 3 days ago
```
**Why it matters:**
- AI receives semantic context automatically — no need to paste code into a chat box
- Suggestions are aware of *where* you are in the code structure
- Reduces the #1 developer complaint: "I have to explain my code to the AI every time"

**Implementation:** Tree-sitter node at cursor position → walk up AST → collect scope chain → feed to model as structured context.

---

### 2.2 Structured Edit History with AI Authorship
**What:** Every edit is tagged with its origin.  
**Edit types:**
- `HUMAN` — developer typed it
- `AI_SUGGESTED` — AI wrote it, developer accepted unchanged
- `AI_MODIFIED` — AI wrote it, developer changed before accepting
- `AI_REJECTED` — AI wrote it, developer dismissed

**Why it matters:**
- "Undo AI suggestion" is a first-class operation — not just Ctrl+Z spam
- Edit history becomes a training signal: what AI suggestions do developers accept?
- Authorship is stored in the rope's mutation log — not a separate system
- Over time, the acceptance pattern personalizes suggestions to each developer

**Competitors:** No editor has AI authorship in the core edit history. They track accept/reject as telemetry logs, not as part of the document model.

---

### 2.3 Streaming Tokens Directly into Rope
**What:** When the model generates code, tokens stream directly into the rope — not into a "ghost text" overlay.  
**How traditional editors do it (Copilot, Cursor):**
```
Model generates text → shown as ghost text overlay
                     → if accepted: text is inserted into buffer
                     → if rejected: overlay is discarded
```
**How ours works:**
```
Model streams token → rope receives it directly
                    → Tree-sitter re-parses incrementally
                    → AST stays valid during streaming
                    → Syntax highlighting updates in real time
```
**Why it matters:**
- No visual glitch — code appears as real text from the first token
- AST is valid throughout streaming — can run Z3 verification on partial output
- Rejected suggestion: rope rolls back using mutation log — no orphaned state

---

### 2.4 Incremental KV Cache Updates (PIE)
**What:** When the developer edits code, only the changed bytes are sent to the model — not the full file.  
**The problem it solves:** Normally, every keystroke requires the LLM to re-encode its entire KV cache to re-predict. For large files this is expensive and slow.  
**The solution — Positional Integrity Encoding (PIE):**
```
Developer edits byte 402 in a 50,000 byte file
    ↓
Rope knows exactly what changed (it's the mutation log)
    ↓
Tree-sitter knows exactly which AST nodes changed
    ↓
Only the delta is sent to the model
    ↓
KV cache is patched, not rebuilt
    ↓
Response latency: dramatically reduced
```
**Why it matters:** This is the difference between sub-100ms suggestions and 500ms suggestions. At that latency gap, it feels like a different product.

**Source:** *"Let the Code LLM Edit Itself When You Edit the Code"* — arXiv:2407.03157

---

## 3. Proactive Intelligence

> AI that acts without being asked. The shift from reactive to anticipatory.

### 3.1 Next Edit Prediction (NEP)
**What:** The editor watches your sequence of edits and predicts *where* your next edit will be and *what* it will contain — before you get there.  

**The gap it fills:**
- Code completion: only suggests at the cursor position
- Chat AI: requires you to stop, describe intent in natural language, context-switch
- NEP: predicts next edit from your behavior, surfaces it via Tab — zero interruption

**How it works:**
```
Developer renames parameter in function signature
    ↓
NEP detects: "rename in progress"
    ↓
Scans call sites using AST (Tree-sitter)
    ↓
Predicts: "next edit will be at line 87, col 14 — same parameter name"
    ↓
Tab → accepts. No prompt. No chat. No context switch.
```

**Real-world benchmark (Ant Group, 20,000 developers):**
- Location prediction accuracy: **75.6%**
- Edit acceptance rate: **43.44%**
- Suggestion latency: **< 250ms**

**Types of edits NEP handles:**
- API updates (rename method across callers)
- Type/object changes (update all usages)
- Identifier renaming (propagate through scope)
- Refactoring patterns (extract function, inline variable)

**Source:** *"Next Edit Prediction"* — arXiv:2508.10074; *"NES: Next Edit Suggestion"* — arXiv:2508.02473; *"CoEdPilot"* — ISSTA '24

---

### 3.2 Zero-Prompt Intent Modeling
**What:** The editor builds a continuous model of *what the developer is trying to accomplish* by watching their edit trajectory — no natural language required.  

**How it works:**
```
Mutation log:
  T+0s:  renamed variable "data" → "formData" in parseForm()
  T+3s:  updated return type annotation
  T+8s:  opened forms.py (the caller)
  T+9s:  cursor moved to line 34 (call site)

Intent model: "developer is propagating a rename through the call chain"
Confidence: 91%

Action: suggest the next rename location + content
        surface as inline hint, not a chat popup
```

**Why this is different from autocomplete:** Autocomplete predicts the next *token*. Intent modeling predicts the next *goal*. These operate at completely different abstraction levels.

**Cognitive load research basis:** Studies show the #1 struggle with AI coding tools is having to *articulate intent in natural language*. Intent modeling eliminates that entirely for common refactoring patterns.

**Source:** *"Towards Decoding Developer Cognition"* — arXiv:2501.02684; *"Prompting LLMs for Code Editing: Struggles and Remedies"* — arXiv:2504.20196

---

### 3.3 Suggestion Confidence Gating
**What:** Suggestions are only shown to the developer when the model's acceptance probability prediction crosses a threshold.  

**The problem it solves:** GitHub Copilot research found developers reject **70% of suggestions**. Every rejected suggestion:
- Breaks the developer's cognitive flow
- Costs ~1-2 seconds of attention
- Trains the developer to ignore suggestions ("notification blindness")

**How gating works:**
```
Model generates suggestion
    ↓
Confidence model evaluates:
  - How similar is this to developer's accepted edits?
  - Is the AST context a known good pattern?
  - How many times has this type of suggestion been rejected?
    ↓
If confidence > threshold → show suggestion
If confidence < threshold → stay silent
```

**Personalization loop:**
- Every accept/reject is stored in the rope's edit history (with AI authorship tag)
- Confidence threshold adjusts per developer over time
- After ~1 week of use: acceptance rate improves significantly

**Source:** *"Predicting Developer Acceptance of AI-Generated Code Suggestions"* — arXiv:2601.21379

---

### 3.4 Flow State Detection
**What:** The editor detects whether the developer is in "flow" (high velocity, focused) or "stuck" (pauses, repeated undos, idle).  

**Behavior:**
- **In flow:** suppress all proactive suggestions, only respond to explicit requests
- **Stuck:** proactively surface relevant context, suggest next steps, offer to explain
- **Context switch:** (e.g., opens a new file) — reload relevant palace memory for that module

**Signals used:**
- Keystroke velocity
- Undo/redo frequency
- Cursor idle time
- File switch frequency
- Repeated failed completions

**Source:** *"Towards Decoding Developer Cognition"* — arXiv:2501.02684

---

## 4. Formal Verification Layer

> Using mathematical proof to validate code — not just testing it.  
> Inspired by Z3 (Microsoft Research) and Noyron's deterministic physics modeling.

### 4.1 Z3 Integration — Live Correctness Proofs
**What:** Z3 is Microsoft's Satisfiability Modulo Theories (SMT) solver — a tool that can *prove* properties about code mathematically, not just test them.  

**What Z3 can prove, inline as you type:**
- "This function can never divide by zero"
- "This loop always terminates"
- "This value is always within bounds [0, 100]"
- "These two conditions can never both be true simultaneously"
- "This function's return value always satisfies the postcondition"

**How it integrates with our editor:**
```
Developer writes a function
    ↓
Tree-sitter parses it → we have the AST
    ↓
AST nodes → translated to Z3 constraints
    ↓
Z3 runs: "Is this constraint satisfiable?"
    ↓
UNSAT → proved safe → green indicator
SAT   → found a counterexample → show it inline
    ↓
AI suggests a fix
    ↓
Fix is verified by Z3 before being shown to developer
    ↓
Developer only sees suggestions that are mathematically correct
```

**Why nobody else ships this:** They don't own the AST. Translating from raw text to Z3 constraints is unreliable. Our AST is the source of truth — translation is clean and deterministic.

**Practical scope (what to verify first):**
- Null pointer dereferences
- Array out-of-bounds access
- Integer overflow
- Unreachable code detection
- Type constraint violations

**Source:** Z3 — Microsoft Research (github.com/Z3Prover/z3); *"Z3: An Efficient SMT Solver"* — de Moura & Bjørner, 2008

---

### 4.2 AI Suggestion Verification Gate
**What:** Before a suggestion is shown to the developer, it passes through Z3.  

```
Model generates code suggestion
    ↓
Suggestion is parsed by Tree-sitter → AST
    ↓
AST → Z3 constraints
    ↓
Z3 verifies: does this suggestion satisfy the constraints of the surrounding code?
    ↓
YES: show suggestion with a "✓ Verified" indicator
NO:  model is asked to regenerate with the failing constraint as additional context
    ↓
Developer only sees suggestions that pass formal verification
```

**This is the key differentiator:** Cursor shows you suggestions and *hopes* they're right. We *prove* they're right before you see them.

---

### 4.3 Deterministic Logic Encoding (Noyron Approach)
**What:** Inspired by LEAP 71's Noyron — encode domain knowledge as deterministic, physics-grounded logic rather than LLM prompts.  

**The Noyron insight:** LEAP 71 built a system that designs rocket engines autonomously. Their key finding: encoding first-principles physics as deterministic computational logic produces *first-time-right* results. In 18 months, they went from specification to hot-fired engine reliably, because the system doesn't guess — it computes.

**Applied to code:** Instead of asking an LLM "how should I structure this API endpoint?" we encode:
- REST conventions as rules (not training data patterns)
- Security constraints as logic (not suggestions)
- Type system rules as formal specifications (not heuristics)
- Language-specific idioms as deterministic functions (not probability distributions)

**What this means in practice:**
```
Developer: "add authentication to this endpoint"

LLM approach (competitors):
  → generates code based on training data patterns
  → may or may not follow your specific framework's conventions
  → may or may not handle edge cases correctly
  → developer must review carefully

Deterministic logic approach (ours):
  → looks up authentication patterns for your specific framework (encoded)
  → applies your project's existing auth middleware pattern (from AST)
  → Z3 verifies the result satisfies security constraints
  → developer can trust the output
```

**Source:** LEAP 71 / Noyron — leap71.com; *"Why engineering must move beyond CAD"* — Metal AM Journal, Jan 2026

---

## 5. Repository Intelligence

> Understanding the whole codebase — not just the open file.

### 5.1 Live Knowledge Graph
**What:** A continuously-maintained graph of the entire codebase — functions, types, modules, call relationships, data flows.  

**How competitors do it (VS Code, Cursor):**
```
Batch process → read files as text → chunk → embed → store in vector DB
                                                     (done every few minutes)
```
Result: a *snapshot* of your codebase. Stale the moment you save a file.

**How ours works:**
```
Developer edits a function
    ↓
Rope records mutation at byte offset 402
    ↓
Tree-sitter re-parses only the changed node
    ↓
AST diff computed: "function parseForm() changed its return type"
    ↓
Knowledge graph updated instantly:
  - parseForm → returns → FormRunResponse (was: FormData)
  - all callers of parseForm flagged for type mismatch check
  - Z3 re-verifies affected call sites
```

**Graph structure:**
```
Nodes:   functions, classes, types, modules, variables, constants
Edges:   calls, imports, inherits, implements, uses, returns, accepts
Tags:    last_modified, author(human|ai), test_coverage, complexity
```

**What the AI gets from this:**
- "What does this function call?"
- "Who calls this function?"
- "If I change this type, what breaks?"
- "What's the blast radius of this refactor?"

**Source:** *"Knowledge Graph Based Repository-Level Code Generation"* — arXiv:2505.14394; *"RACG Survey"* — arXiv:2510.04905

---

### 5.2 Repository-Level RAG (Retrieval-Augmented Generation)
**What:** When the AI needs context beyond the current file, it retrieves relevant code from across the entire repo.  

**The problem with naive RAG (what others do):**
- Split files into fixed-size text chunks (e.g., every 512 tokens)
- Embed chunks into vectors
- Find similar chunks by cosine distance

**Problems:** Chunks cut through function boundaries. A function split across two chunks loses meaning. A class split across three chunks loses structure.

**Our approach — AST-native chunking:**
```
Tree-sitter parses each file
    ↓
Chunk boundaries = AST node boundaries
  (functions, classes, methods — never mid-statement)
    ↓
Each chunk is a semantically complete unit
    ↓
Embedding captures the full meaning of the chunk
    ↓
Retrieval returns complete, meaningful code units
```

**Dual index:**
- Vector index (ChromaDB or similar) — semantic similarity search
- Graph index (knowledge graph) — structural/relational search

**Combined query:** "Find functions that: (a) are semantically similar to what I'm writing, AND (b) are called by modules in the same layer as my current file"

**Source:** *"Context Engineering for Multi-Agent LLM Code Assistants"* — arXiv:2508.08322; *"RACG Survey"* — arXiv:2510.04905

---

### 5.3 AST-Level Edit Planning (Plan Mode)
**What:** Before executing any multi-file change, the editor generates and displays a structured plan — in AST terms, not line diffs.  

**How it looks:**
```
Request: "Add authentication to all public API endpoints"

PLAN (generated before touching any file):
├── Scan module: api/routes/
│   ├── Found 7 public endpoints (no auth decorator)
│   └── Found 2 already authenticated endpoints (skip)
│
├── Changes to make:
│   ├── api/routes/user.py
│   │   ├── function get_users()  → add @require_auth decorator
│   │   └── function create_user() → add @require_auth decorator
│   ├── api/routes/data.py
│   │   └── function export_data() → add @require_auth decorator
│   └── tests/test_api.py
│       └── Add auth headers to 5 existing test cases
│
├── Estimated impact:
│   ├── Files modified: 3
│   ├── Functions changed: 5
│   └── Tests affected: 5
│
└── Z3 verification: all changes pass auth constraint checks ✓

[APPROVE] [MODIFY] [CANCEL]
```

**Why this matters:**
- Developer reviews the *intent* before any code changes
- Plan is AST-level — you see function names, not line numbers
- Approval is one click — execution is automatic
- Any step can be excluded from the plan before running

**Source:** *"A Survey on Code Generation with LLM-based Agents"* — arXiv:2508.00083

---

## 6. Persistent Memory System

> AI that remembers across sessions — without forgetting the reasoning, only the conclusion.  
> Inspired by MemPalace (Milla Jovovich + Ben Sigman, April 2026 — 34,000 GitHub stars in 72 hours).

### 6.1 The Core Problem
Every time a developer opens the editor, the AI has amnesia:
- Why was this function written this way?
- What alternatives were considered and rejected?
- What broke when we tried approach X last month?
- What's the current status of the auth migration?

Existing solutions (Mem0, Zep) use AI to *summarize* conversations → **lossy**. The reasoning gets discarded. Only the conclusion survives.

---

### 6.2 MemPalace Principle — Store Everything, Make It Findable
**The key insight from MemPalace research:**

> "The field is over-engineering the memory extraction step. Raw verbatim text with good embeddings is a stronger baseline than anyone realized — because it doesn't lose information."

**Benchmark results (LongMemEval):**
| System | Recall@5 |
|---|---|
| Mem0 | ~48% |
| Zep | ~52% |
| MemPalace (raw) | **96.6%** |
| MemPalace (hybrid) | **100%** |

The difference: Mem0 and Zep decide what to remember. MemPalace keeps everything and makes it findable.

---

### 6.3 Palace Architecture for Code
The ancient "method of loci" — memory palace technique — applied to codebase memory:

```
PALACE
│
├── Wing: "NexAssist"                     ← project/repo
│   ├── Hall: decisions                   ← memory type
│   │   ├── Room: "oracle-auth"           ← topic
│   │   │   └── "Decided j_security_check over REST auth
│   │   │        because REST wasn't supported on-premise.
│   │   │        Tested: 2024-11-03. Confirmed working."
│   │   └── Room: "export-slice-design"
│   │       └── "Chose pagination by page dimension
│   │            over date range — date range had gaps
│   │            in sparse datasets. Discovered 2024-12-01."
│   ├── Hall: problems
│   │   └── Room: "refresh-token-bug"
│   │       └── "undefined stored as string in localStorage.
│   │            Root cause: bad git rebase introduced mock login.
│   │            Fixed: 2025-01-15. See commit a3f2b1."
│   └── Hall: patterns
│       └── Room: "form-parsing"
│           └── "Forms use per-axis dimension member storage.
│                row_dim_members, col_dim_members arrays.
│                XML → parseForm → FormRunResponse."
│
├── Wing: "FilmyWeds"
│   └── ...
│
└── Tunnels (cross-references):
    "auth-migration" room appears in both NexAssist and FilmyWeds wings
    → tunnel connects them → search one finds both
```

**Retrieval accuracy by scope (from MemPalace research):**
```
Flat search (all closets):     60.9%
+ Wing scope:                  73.1%  (+12%)
+ Hall scope:                  84.8%  (+24%)
+ Wing + Room scope:           94.8%  (+34%)
```

The structure is not cosmetic — it's a **34% retrieval improvement**.

---

### 6.4 Your Editor's Advantage Over MemPalace
MemPalace infers structure from raw chat logs using regex patterns:
- 20 patterns to detect decisions ("let's use", "because")
- 16 patterns to detect preferences
- 33 patterns to detect milestones

**Your editor already has the structure from Tree-sitter:**
```
MemPalace infers:   "user seems to be working on authentication"
Your editor knows:  cursor is inside function validateToken()
                    in file auth/middleware.ts
                    which calls jwtVerify() from oracle module
                    last edited 3 days ago by [human]
                    test coverage: 72%
```

The palace rooms map directly to AST nodes — no inference needed.

| MemPalace Concept | Editor Equivalent |
|---|---|
| Wings | Repositories / modules |
| Halls (memory types) | Decisions / bugs / patterns / tests |
| Rooms (topics) | Functions / classes / features |
| Closets | Edit history segments (timestamped) |
| Tunnels | AST call graph edges (Tree-sitter) |

---

### 6.5 Session Memory Layers
```
L0 (~50 tokens)   → always loaded: active repo, current file, dev preferences
L1 (~120 tokens)  → always loaded: recent decisions in current module
L2+               → on-demand: palace search when AI needs deeper context
```
AI "wakes up" informed in **170 tokens**. No full context dump. Searches only when needed.

---

### 6.6 Temporal Entity Graph
**What:** SQLite-backed graph of: what changed, when, and why.  
**Enables queries like:**
- "What was this function doing 2 weeks ago?"
- "When did we introduce this dependency?"
- "What changed after the auth bug was fixed?"
- "Show me all decisions made about the export module"

---

## 7. Computational Engineering Model

> The Noyron lesson applied to software: encode knowledge as deterministic logic, not as LLM training data.

### 7.1 The Noyron Insight
LEAP 71's Noyron designed and hot-fired a working 20kN rocket engine in under 3 weeks — from specification to hardware — without human intervention.

Their method: encode first-principles physics, engineering heuristics, manufacturing constraints, and real-world test feedback into a **deterministic computational model**. Not a generative model. Not pattern matching. Logic.

Result: *first-time-right* designs. No guessing. No hallucination. Mathematically sound output.

---

### 7.2 Applied to Code: Domain-Encoded Rules
**Instead of:** asking LLM to generate REST endpoint code from training data patterns  
**Do:** encode REST conventions as deterministic rules that the LLM applies

**Examples of encodable knowledge:**
```
Rule: HTTP_METHOD_SEMANTICS
  GET    → must be idempotent, no side effects, cacheable
  POST   → creates resource, returns 201 + Location header
  PUT    → replaces resource, idempotent
  DELETE → idempotent, returns 204

Rule: AUTH_REQUIREMENT
  Any endpoint without @require_auth AND not in PUBLIC_ROUTES
  → flagged as potential security violation
  → Z3 constraint: ∀ endpoint e, e ∈ PUBLIC_ROUTES ∨ has_auth(e)

Rule: ERROR_HANDLING
  Any function that calls external service
  → must have try/catch
  → must log the error
  → must return appropriate HTTP status code
```

These rules are checked against the AST. Violations surface inline. AI suggestions that violate them are rejected before being shown.

---

### 7.3 Feedback Loop (Real-World Enrichment)
Noyron's power comes from its feedback loop: test a physical engine → collect real data → feed back into the model → improve.

**Our equivalent:**
```
Developer accepts AI suggestion
    ↓
Code runs in tests / production
    ↓
Test failures, runtime errors, PR review comments collected
    ↓
Fed back into the suggestion model
    ↓
Next suggestion of the same type is better
```

This is what turns a static code completion system into a continuously improving engineering model.

---

## 8. Collaboration & Agent Protocol

### 8.1 Real-Time Collaboration (CRDT-based)
**What:** Multiple developers and AI agents edit the same file simultaneously without conflicts.  
**Technology:** CRDT (Conflict-free Replicated Data Types) — same technology as Google Docs and Figma.  
**Key difference from VS Code Live Share:**
- Live Share is a plugin — complex setup, frequent connection issues
- Ours is in the editor's DNA — no setup, no connection management

**AI as a participant:** The AI agent is a first-class collaborator in the session — not a sidebar chatbot. Its edits appear in real time alongside human edits, with AI authorship tagging.

---

### 8.2 Agent Client Protocol (ACP)
**What:** An open protocol (pioneered by Zed + JetBrains in January 2026) that lets external AI agents plug into any compatible editor.  
**Why it matters:** You don't have to build every agent yourself. Claude Code, Codex, OpenCode — they all plug in via ACP.  
**Our advantage:** Because we own the rope + AST, agents have richer context than they get in Zed. They see the semantic cursor, the knowledge graph, the palace memory — not just file contents.

---

### 8.3 MCP (Model Context Protocol) Integration
**What:** Connect the editor to external tools: databases, APIs, GitHub, filesystems, documentation.  
**Examples:**
- Agent queries your Oracle EPM instance directly from the editor
- Agent reads your API documentation and generates conforming code
- Agent checks GitHub issues before suggesting a fix
- Agent queries your test database to generate realistic test data

**Permission model:** Every tool call requires explicit approval, or can be auto-approved per tool per session.

---

### 8.4 Parallel Agent Execution
**What:** Dispatch multiple agents to work on different tasks simultaneously.  
**Example:**
```
You: "While I fix this bug, do the following in parallel:"

Agent 1: Refactor the authentication module (isolated workspace)
Agent 2: Write tests for the forms parser
Agent 3: Update the API documentation

All three work simultaneously.
You review diffs when they complete.
```

**Key difference from Z3N's Manager View:** Our parallel execution is AST-aware. Agents know which functions they've claimed. If Agent 1 and Agent 3 both need to touch auth.py, the system coordinates — no merge conflict.

---

## 9. Developer Experience

### 9.1 Autonomy Slider
**What:** Per-session control over how much the AI acts on its own.

```
MANUAL ←————————————————————————→ AUTONOMOUS
  │                                        │
Tab       Inline      File      Repo      Full
complete  edits       agent     agent     auto
```

- **Tab complete:** AI suggests next token/line only
- **Inline edits:** AI applies changes in the current function
- **File agent:** AI can modify the current file autonomously
- **Repo agent:** AI can modify any file, with approval gates
- **Full auto:** AI works with minimal interruption (for well-defined tasks)

---

### 9.2 AI Edit Provenance View
**What:** Any line in the editor can be queried: "who wrote this and why?"

```
Right-click → "Show provenance"

Line 47: return FormRunResponse(...)
  ├── Written by: AI (Claude Sonnet 4.6)
  ├── Suggested at: 2025-11-03 14:22
  ├── Developer action: accepted unchanged
  ├── Confidence: 94%
  └── Based on: parseForm() return type from oracle_form.py
```

---

### 9.3 Multibuffer View
**What:** Compose excerpts from across the entire codebase into one editable surface.  
**Example:** See `parseForm()` definition + all callers + all related tests — in one unified view. Edit any of them; changes propagate back to source files.

---

### 9.4 Cognitive Load Indicators
**What:** Visual indicators of code complexity, AI confidence, and verification status.

```
Line indicator:
  🟢 Verified by Z3 — proven correct
  🟡 AI-written — review recommended
  🔴 Constraint violation detected
  ⚪ Human-written, unverified
```

---

### 9.5 Multi-Turn Structured Synthesis
**What:** Complex tasks are broken into a structured multi-turn conversation tied to the AST — not a free-form chat.

```
Task: "Add pagination to the export endpoint"

Turn 1: AI generates plan (AST-level)
Turn 2: Developer modifies plan ("skip the page size parameter")
Turn 3: AI implements modified plan
Turn 4: Z3 verifies the implementation
Turn 5: AI writes tests
Turn 6: Developer approves
```

Each turn is a structured operation on the AST — not a text exchange that gets fed back as a long prompt.

**Source:** *"CodeGen: Multi-Turn Program Synthesis"* — arXiv:2203.13474

---

## 10. Competitive Comparison

| Feature | VS Code + Copilot | Cursor | Zed | Z3N | **Ours** |
|---|---|---|---|---|---|
| Built from scratch | ❌ | ❌ | ✅ | ❌ (VSCode fork) | ✅ |
| Live AST as AI context | ❌ | ❌ | Partial | ❌ | ✅ |
| Semantic cursor | ❌ | ❌ | ❌ | ❌ | ✅ |
| AI authorship in edit history | ❌ | ❌ | ❌ | ❌ | ✅ |
| Streaming tokens → rope | ❌ | ❌ | ❌ | ❌ | ✅ |
| Incremental KV cache (PIE) | ❌ | ❌ | ❌ | ❌ | ✅ |
| Next Edit Prediction | ❌ | Partial | ❌ | ❌ | ✅ |
| Zero-prompt intent modeling | ❌ | ❌ | ❌ | ❌ | ✅ |
| Suggestion confidence gating | ❌ | ❌ | ❌ | ❌ | ✅ |
| Z3 formal verification | ❌ | ❌ | ❌ | ❌ | ✅ |
| Verified AI suggestions | ❌ | ❌ | ❌ | ❌ | ✅ |
| Live knowledge graph | ❌ | ❌ | ❌ | ❌ | ✅ |
| AST-chunked RAG | ❌ | ❌ | ❌ | ❌ | ✅ |
| AST-level edit planning | ❌ | ❌ | ❌ | ❌ | ✅ |
| Palace memory system | ❌ | ❌ | ❌ | ❌ | ✅ |
| Temporal entity graph | ❌ | ❌ | ❌ | ❌ | ✅ |
| CRDT collaboration | ❌ | ❌ | ✅ | ❌ | ✅ |
| ACP support | ❌ | ❌ | ✅ | ❌ | ✅ |
| MCP support | ✅ | ✅ | ✅ | ❌ | ✅ |
| Multi-model support | ✅ | ✅ | ✅ | Partial | ✅ |
| Local model support | ❌ | ❌ | ✅ | ❌ | ✅ |

---

## 11. Research References

### Papers

| Paper | arXiv / Venue | Feature |
|---|---|---|
| Next Edit Prediction | arXiv:2508.10074 | NEP — proactive edit suggestions |
| NES: Next Edit Suggestion | arXiv:2508.02473 | Instruction-free, 250ms, 75.6% accuracy |
| CoEdPilot | ISSTA '24 | Edit propagation, cross-file awareness |
| RACG Survey | arXiv:2510.04905 | Repository-level RAG for code |
| Knowledge Graph Repo Code Gen | arXiv:2505.14394 | Live KG architecture |
| Context Engineering Multi-Agent | arXiv:2508.08322 | AST-chunked retrieval pipeline |
| Let Code LLM Edit Itself (PIE) | arXiv:2407.03157 | Incremental KV cache updates |
| Predicting Developer Acceptance | arXiv:2601.21379 | Confidence gating, 70% rejection rate |
| Decoding Developer Cognition | arXiv:2501.02684 | Flow state, intent modeling |
| Prompting LLMs for Code Editing | arXiv:2504.20196 | Intent communication cognitive load |
| LLM Agent Code Gen Survey | arXiv:2508.00083 | Structured planning before codegen |
| CodeGen Multi-Turn Synthesis | arXiv:2203.13474 | Multi-turn structured synthesis |
| AI IDEs vs Autonomous Agents | MSR '26 (arXiv:2601.13597) | Agentic vs IDE-based paradigm analysis |
| Z3: An Efficient SMT Solver | TACAS 2008 (de Moura & Bjørner) | Formal verification foundation |

### Industry Sources

| Source | Key Contribution |
|---|---|
| MemPalace (Jovovich + Sigman, Apr 2026) | Verbatim memory, 96.6% recall, hierarchical palace architecture |
| LEAP 71 / Noyron (2023–2026) | Deterministic computational engineering model, first-time-right design |
| Zed (2026) | CRDT collaboration, ACP protocol, Zeta2 intent model, GPUI GPU rendering |
| Cursor v3.0 (2026) | Background agents, BugBot self-improvement (78% resolution), cloud agents |
| Google Z3N (Nov 2025) | Manager View, parallel agents, browser subagents, artifact trust model |
| VS Code 1.110 (Feb 2026) | Agent memory across sessions, context compaction, multi-agent orchestration |
| Ant Group NES Deployment | 51.55% location acceptance, 43.44% edit acceptance across 20,000 devs |
| GitHub Copilot Study (Ziegler et al.) | 70% suggestion rejection rate — confidence gating essential |
| Google Internal IDE Study (Jan 2026) | Productivity gains from code completion + transform code features |

---

*Version: 2.0*  
*Last updated: April 2026*  
*Status: Living document — add features as research continues*  
*Next review: After Z3 integration prototype*