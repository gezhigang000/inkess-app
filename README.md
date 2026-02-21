<p align="center">
  <img src="src-tauri/icons/icon.png" width="128" height="128" alt="Inkess" />
</p>

<h1 align="center">Inkess</h1>

<p align="center">
  A modern, lightweight Markdown reader & editor for desktop.<br/>
  现代、轻量的桌面 Markdown 阅读器与编辑器。
</p>

<p align="center">
  <a href="#english">English</a> · <a href="#中文">中文</a>
</p>

---

<a id="english"></a>

## Features

- **Markdown Editing & Preview** — Live preview with GitHub-flavored Markdown, code highlighting, and multiple export formats (PDF / HTML / Image)
- **Multi-format Viewer** — Built-in support for PDF, DOCX, XLSX, images, and 30+ code/config file types
- **AI Assistant** — OpenAI-compatible LLM integration with streaming, tool use (file read/write, web search, Python execution), deep thinking mode, and MCP client support
- **Built-in Terminal** — Multi-tab PTY terminal with provider management (environment variable injection), color scheme presets, and session logging
- **Git Integration** — Visual Git status, staging, commit, push/pull with multi-remote support
- **File Search** — Fast recursive file search (Rust-powered, 8 levels deep, 50 results max)
- **Document Search** — In-document Cmd+F search with highlight navigation
- **Snapshot System** — SQLite-based file snapshots with configurable retention policies
- **RAG Knowledge Base** — Local ONNX embedding + sqlite-vec vector search for document retrieval
- **Theme System** — Multiple themes (GitHub / Minimal / Dark) with terminal color scheme presets (Solarized Dark / Nord / Catppuccin / Rosé Pine)
- **i18n** — English and Chinese, switchable at runtime
- **Freemium Model** — Free tier with full basic features; Pro unlocks AI, terminal, Git, and unlimited snapshots

## Tech Stack

- **Frontend**: React 19 + TypeScript + Vite
- **Backend**: Tauri 2.x + Rust
- **Terminal**: portable-pty + xterm.js
- **AI**: OpenAI-compatible API, SSE streaming, tool use
- **Storage**: SQLite (snapshots) + sqlite-vec (RAG vectors) + localStorage
- **Embedding**: ONNX Runtime (ort) with local models

## Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/) >= 18
- [Rust](https://rustup.rs/) >= 1.75
- Platform-specific dependencies for Tauri: [Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/)

### Development

```bash
npm install
npm run tauri dev
```

### Build

```bash
npm run tauri build
```

Build artifacts will be in `src-tauri/target/release/bundle/`.

### Project Structure

```
src/                    # React frontend
  components/           # UI components
  lib/                  # Utilities (tauri bindings, markdown, export, themes, i18n)
  themes/               # Theme CSS files
src-tauri/              # Rust backend
  src/lib.rs            # Main entry + file/snapshot commands
  src/ai.rs             # AI assistant (LLM client, SSE, tool use)
  src/pty.rs            # PTY terminal management
  src/git.rs            # Git operations
  src/rag/              # RAG knowledge base (indexer, embedding, vector store)
  src/mcp/              # MCP client (JSON-RPC, stdio/HTTP transport)
  src/license.rs        # License verification
  src/session_logger.rs # Terminal session logging
```

## License

MIT

---

<a id="中文"></a>

## 功能特性

- **Markdown 编辑与预览** — 实时预览，支持 GFM 语法、代码高亮，多格式导出（PDF / HTML / 图片）
- **多格式查看器** — 内置 PDF、DOCX、XLSX、图片及 30+ 种代码/配置文件类型支持
- **AI 助手** — 兼容 OpenAI 接口的 LLM 集成，支持流式输出、工具调用（文件读写、联网搜索、Python 执行）、深度思考模式、MCP 客户端
- **内置终端** — 多标签 PTY 终端，Provider 管理（环境变量注入）、配色方案预设、会话日志记录
- **Git 集成** — 可视化 Git 状态、暂存、提交、推送/拉取，支持多 remote
- **文件搜索** — Rust 驱动的快速递归搜索（最深 8 层，最多 50 条结果）
- **文档内搜索** — Cmd+F 文档内搜索，高亮导航
- **快照系统** — 基于 SQLite 的文件快照，可配置保留策略
- **RAG 知识库** — 本地 ONNX 嵌入向量 + sqlite-vec 向量搜索，支持文档检索
- **主题系统** — 多主题（GitHub / Minimal / Dark），终端配色预设（Solarized Dark / Nord / Catppuccin / Rosé Pine）
- **多语言** — 中英文双语，运行时切换
- **Freemium 模式** — 免费版包含完整基础功能；Pro 版解锁 AI、终端、Git、无限快照

## 技术栈

- **前端**: React 19 + TypeScript + Vite
- **后端**: Tauri 2.x + Rust
- **终端**: portable-pty + xterm.js
- **AI**: OpenAI 兼容接口，SSE 流式，工具调用
- **存储**: SQLite（快照）+ sqlite-vec（RAG 向量）+ localStorage
- **嵌入模型**: ONNX Runtime (ort)，本地推理

## 快速开始

### 环境要求

- [Node.js](https://nodejs.org/) >= 18
- [Rust](https://rustup.rs/) >= 1.75
- Tauri 平台依赖：[Tauri 环境配置](https://v2.tauri.app/start/prerequisites/)

### 开发模式

```bash
npm install
npm run tauri dev
```

### 构建发布

```bash
npm run tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`。

### 项目结构

```
src/                    # React 前端
  components/           # UI 组件
  lib/                  # 工具库（tauri 绑定、markdown、导出、主题、i18n）
  themes/               # 主题 CSS
src-tauri/              # Rust 后端
  src/lib.rs            # 主入口 + 文件/快照命令
  src/ai.rs             # AI 助手（LLM 客户端、SSE、工具调用）
  src/pty.rs            # PTY 终端管理
  src/git.rs            # Git 操作
  src/rag/              # RAG 知识库（索引、嵌入、向量存储）
  src/mcp/              # MCP 客户端（JSON-RPC、stdio/HTTP 传输）
  src/license.rs        # License 验证
  src/session_logger.rs # 终端会话日志
```

## 开源协议

MIT
