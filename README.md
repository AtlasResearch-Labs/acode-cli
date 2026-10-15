# Atlas Daedalus (`acode`)
> Industrial Autonomous Terminal AI Coder · Sovereign Pure Rust Engine · Atlas Research Labs

Atlas Daedalus is an industrial-grade, terminal-native AI software engineering agent designed to supersede heavy, cloud-locked coding assistants (Claude Code, Cursor CLI, OpenAI Codex CLI).

Built in 100% pure native Rust with zero V8 runtime overhead, Daedalus executes with sub-millisecond cold start times, embedded Ripgrep search, automated test self-healing loops, and strict zero-emoji OLED monochrome precision.

---

## Technical Specifications

| Metric | Claude Code / Cursor CLI | Atlas Daedalus (`acode`) |
|---|---|---|
| Engine Architecture | TypeScript / Python / Node.js | **Pure Native Rust (`x86_64`, `aarch64`)** |
| Memory Footprint (RSS) | 150MB - 350MB | **< 8MB RSS** |
| Cold-Start Latency | 350ms - 800ms | **< 5ms** |
| Codebase Search | Spawns external shell / regex | **Embedded Ripgrep Engine (`ignore::WalkBuilder`)** |
| File Diff Generation | Spawns external git process | **Native In-Memory (`similar`) Unified Diffs** |
| Automated Self-Healing | Brittle / prompt-wrapped | **Autonomous loop (`cargo test`, `npm test`, `pytest`)** |
| Platform Coverage | Desktop only | **Windows, Linux, macOS, Android (Termux)** |
| Interface Philosophy | Cartoonish / bloated emojis | **Strict Zero-Emoji OLED Monochrome ANSI** |

---

## Universal Installation

Daedalus ships with pre-compiled static native binaries for Windows, Linux, and Android (Termux aarch64). Rust is NOT required on client devices.

### Method 1: NPM / NPX (Zero Setup)

Run instantly without global installation:
```bash
# Interactive REPL
npx acode-cli

# Autonomous task execution
npx acode-cli "implement jwt authentication and verify tests pass"
```

Or install globally across any platform:
```bash
npm install -g acode-cli
acode
```

### Method 2: Android Termux (Mobile)

On Android Termux, install Node.js and run directly:
```bash
pkg install nodejs
npm install -g acode-cli
acode
```
The package automatically executes the pre-compiled `linux-aarch64` native engine with zero compilation needed.

### Method 3: Cargo (From Source)

```bash
cargo install --path .
```

---

## AI Providers & Authentication

Daedalus connects seamlessly to sovereign AI routers, multi-model gateways, and local backends.

Configure credentials in `~/.daedalus/.env` or via standard environment variables:

```bash
# Atlas AI-Powerbox (Sovereign Gateway)
export POWERBOX_API_KEY="pbk_live_..."
export POWERBOX_URL="https://powerbox.atlasresearchlabs.online"

# Google Gemini (Default: gemini-2.5-flash)
export GEMINI_API_KEY="AIzaSy..."

# Anthropic Claude (Default: claude-3-5-sonnet)
export ANTHROPIC_API_KEY="sk-ant-..."

# OpenAI (Default: gpt-4o)
export OPENAI_API_KEY="sk-proj-..."

# Local Ollama (100% Offline Sovereign Inference)
export OLLAMA_HOST="http://localhost:11434"
acode -p ollama -m deepseek-coder:6.7b
```

---

## CLI Options

```
Usage: acode [OPTIONS] [PROMPT]...

Arguments:
  [PROMPT]...  Autonomous task prompt to run non-interactively

Options:
  -m, --model <MODEL>        Override active LLM model (e.g. gemini-2.5-flash, gpt-4o)
  -p, --provider <PROVIDER>  Override active AI provider (powerbox, gemini, anthropic, openai, ollama)
  -s, --security <SECURITY>  Override agent security mode (full, sandboxed, strict)
  -h, --help                 Print help
  -V, --version              Print version
```

---

## Interactive REPL Slash Commands

Inside the interactive terminal session, invoke slash commands for instant developer actions:

| Command | Action |
|---|---|
| `/help` | Print cheatsheet and keyboard navigation keys |
| `/diff` | Display color-coded unified git diff of all modifications |
| `/status` | View modified files, token counters, and active engine configuration |
| `/commit` | Generate conventional commit and commit with sovereign author |
| `/cost` | Display session token consumption, network latency, and cost ledger |
| `/audit` | Execute autonomous AST static code review |
| `/tunnel` | Expose active port via Beacon public tunnel |
| `/pulse` | Inspect host CPU, memory, and process health metrics |
| `/model <m>` | Dynamically switch LLM model mid-session |
| `/clear` | Clear terminal viewport and reset trajectory buffer |
| `/exit` | Terminate session |

---

## Repository & Development Standards

Atlas Daedalus adheres to strict engineering invariants:
1. **Zero Emoji Policy**: Absolute prohibition of emojis in all logs, UI strings, commits, and documentation.
2. **Sovereign Authorship**: All git commits are signed by `Eren Yeager <victor@atlasresearchlabs.online>`.
3. **No Credential Leaks**: Credentials, keys, and tokens are strictly excluded via `.gitignore`.
4. **Minimal Footprint**: Single standalone static binary with zero external runtime toolchains.

---

## License

MIT (C) 2026 Atlas Research Labs
