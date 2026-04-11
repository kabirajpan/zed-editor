# Pre-UI Polish Roadmap

This document tracks the core engine features that need to be implemented or refactored before transitioning to UI development (File Tree, Tab System, etc.).

## Core Editing Features

- [x] **Block Indentation**
  - [x] Tab to indent multi-line selection.
  - [x] Shift+Tab to outdent multi-line selection.
  - [x] Correctly handle whitespace and partial line selections.

- [x] **Move & Duplicate Lines**
  - [x] `Alt + Up/Down` to swap lines.
  - [ ] `Shift + Alt + Up/Down` to duplicate lines/selections.

- [ ] **Intelligent Editing**
  - [x] **Toggle Comment**: `Ctrl + /` with language-specific comment tokens (Rust: `//`, C++: `//`).
  - [x] **Auto-Closing Pairs**: Quotes (`"`, `'`), Brackets (`(`, `[`, `{`).
  - [x] **Smart Overwrite**: Typing a closing bracket while inside one should skip it.

## Engine Infrastructure

- [ ] **Multi-Buffer Management**
  - [ ] Create a `Workspace` or `BufferManager` to hold multiple `Editor` instances.
  - [ ] Implement switching logic (Focus/Blur).

- [ ] **Search & Replace Logic**
  - [ ] `Find` logic (regex-capable).
  - [ ] `Replace` logic.
  - [ ] `Find in Files` (Project-wide search).

---

## Status Updates
- [x] **Atomic Delta synchronization** (Base Engine)
- [x] **Smart Indentation** (Basic `{` Enter handling)
- [x] **Undo/Redo** (Transactional)
