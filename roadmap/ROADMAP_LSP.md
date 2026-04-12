# Roadmap: Layer 3 - LSP Integration (The Neovim / Mason Paradigm)

To match and exceed a professional Neovim/Mason setup, Z3N will integrate an **LSP (Language Server Protocol)** client. This moves Z3N from "AI-Only" intelligence to a hybrid system using **Local Static Analysis** + **Remote Generative Intelligence**.

## 🔱 Why LSP? (Neovim vs Z3N)
In Neovim, **Mason** manages binaries like `rust-analyzer`. **nvim-lspconfig** connects them. **nvim-cmp** provides the UI. Z3N will implement these three pillars natively.

| Feature | Local LSP (Z3N Layer 3) | Remote AI (Z3N Layer 2) |
| :--- | :--- | :--- |
| **Speed** | Instant (<10ms) | Latent (500ms - 2s) |
| **Reliability** | 100% Type-Safe | Speculative / Hallucinatory |
| **Depth** | Project-wide symbols, Types | Cross-file logic, Intent |

## 🏗️ The 3-Pillar Integration Plan

### 1. The Mason Equivalent: Z3N Binary Manager
Z3N will include a downloader for `rust-analyzer`, `typescript-language-server`, and `pyright`.
- **Status**: Researching `tower-lsp` and `lsp-types` crates for Rust-native client logic.

### 2. The LspConfig Equivalent: Persistent RPC
Z3N will launch LSPs in the background via `std::process::Child` and communicate via JSON-RPC over stdin.
- **Layer 3.1**: "Go to Definition" and "Find References" using LSP.
- **Layer 3.2**: Live Diagnostics (Red squiggles) provided by the compiler, not the AI.

### 3. The nvim-cmp Equivalent: Hybrid Suggestion Engine
The `trigger_nep` logic will be upgraded to be **Hybrid**:
1. **LSP-First**: As you type `x.`, the LSP instantly provides a list of valid methods.
2. **AI-Overlay**: If you pause, the AI takes the LSP context and predicts the most likely *line* of code.

## 🚀 Priority 1: rust-analyzer Integration
Our first target is `rust-analyzer`.
- **Goal**: Full support for `textDocument/completion` and `textDocument/hover`.
- **Architecture**: Z3N will spawn a dedicated thread per LSP to manage the async IO loop without blocking the GUI.

---
**This is the path to making Z3N the fastest, most capable code editor in the world.**
