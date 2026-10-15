# Atlas Daedalus (`acode`)
> Industrial Autonomous Terminal AI Coder · 100% Pure Native Rust Engine · Atlas Research Labs

Atlas Daedalus is a sovereign, terminal-native AI software engineer engineered to replace slow, heavy, cloud-locked coding agents (Claude Code, Cursor CLI, OpenAI Codex CLI).

Built in pure native Rust with zero V8 runtime bloat, Daedalus executes with sub-millisecond cold start, an embedded Ripgrep search engine, automated self-healing test loops, and strict zero-emoji OLED monochrome precision.

---

## Performance Benchmark

| Metric | Claude Code / Cursor CLI | Atlas Daedalus (`acode`) |
|---|---|---|
| Engine | TypeScript / Python / Node.js | **Pure Native Rust** |
| Runtime Footprint | 150MB - 350MB RSS | **< 8MB RSS** |
| Cold-Start Latency | 350ms - 800ms | **< 5ms** |
| Codebase Search | Spawns external shell / regex | **Embedded Ripgrep Engine (`ignore::WalkBuilder`)** |
| In-Memory Diffs | Spawns git subprocess | **Native In-Memory (`similar`) Unified Diffs** |
| Test Self-Healing | Manual / brittle | **Automatic (`cargo test`, `npm test`, `pytest`)** |
| Design Standard | Emojis & cartoonish output | **Strict Zero-Emoji OLED Black ANSI** |

---

## Quickstart

Run directly via `npx` with zero installation:

```bash
# Interactive OLED terminal REPL
npx acode-cli

# Or run non-interactive autonomous tasks
npx acode-cli "implement user authentication module and verify tests pass"
```

Or install globally:
```bash
npm install -g acode-cli
acode
```

---

## Supported Providers

Daedalus supports all major AI models via environment variables or CLI flags:

```bash
# Google Gemini (Default: gemini-2.5-flash)
export GEMINI_API_KEY="your-gemini-key"

# Anthropic Claude (Default: claude-3-5-sonnet)
export ANTHROPIC_API_KEY="your-anthropic-key"

# OpenAI GPT-4o
export OPENAI_API_KEY="your-openai-key"

# Local Ollama (Zero cloud, 100% offline)
export OLLAMA_HOST="http://localhost:11434"
acode -p ollama -m deepseek-coder:6.7b
```

---

## Built-in Slash Commands

When running in interactive REPL mode:

| Command | Action |
|---|---|
| `/help` | Display command cheatsheet and keyboard shortcuts |
| `/diff` | Display color-coded unified git diff of all modifications |
| `/status` | Display modified files, current provider, and active model |
| `/commit` | Generate conventional commit and commit with sovereign author |
| `/cost` | Display session token usage, latency, and Chronicle cost breakdown |
| `/audit` | Run autonomous AST code review via Argus |
| `/tunnel` | Expose active port via Beacon public tunnel |
| `/pulse` | Inspect host CPU, memory, and process health |
| `/model <m>` | Dynamically switch LLM model mid-session |
| `/clear` | Clear terminal screen and reset conversation trajectory |
| `/exit` | Exit Daedalus |

---

## License

MIT © Atlas Research Labs
