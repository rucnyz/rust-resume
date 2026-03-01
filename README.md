# rust-resume (fr-rs)

Fast fuzzy finder TUI for coding agent session history. Search and resume sessions across 10 coding agents.

## Supported Agents

Claude Code, Codex CLI, GitHub Copilot CLI, Copilot VSCode, Crush, Gemini CLI, Kimi CLI, OpenCode, Qwen Code, Vibe

## Install

### Binary (recommended)

```sh
curl -fsSL https://rucnyz.github.io/rust-resume/install.sh | bash
```

Installs to `~/.local/bin/fr-rs`. Supports Linux (x86_64/aarch64) and macOS (Intel/Apple Silicon).

### From source

```sh
# requires Rust toolchain (https://rustup.rs)
cargo install --git https://github.com/rucnyz/rust-resume
```

### Build from scratch

```sh
git clone https://github.com/rucnyz/rust-resume.git
cd rust-resume
cargo build --release
# binary at target/release/fr-rs
```

## Usage

```sh
fr-rs                          # Open TUI
fr-rs --list                   # List sessions to stdout
fr-rs --list 'niri'            # Search and list
fr-rs --agent claude --list    # Filter by agent
fr-rs --rebuild --list         # Force rebuild index
fr-rs --stats                  # Show index stats
```

## Keybindings

| Key | Action |
|-----|--------|
| `↑/↓` or `j/k` | Navigate results |
| `Enter` | Resume selected session |
| `Tab` / `Shift+Tab` | Cycle agent filter |
| `Ctrl+S` | Toggle sort (relevance / time) |
| `Ctrl+U/D` | Scroll preview |
| `` Ctrl+` `` | Toggle preview |
| `Ctrl+P` | Toggle preview layout |
| `c` | Copy resume command |
| `Ctrl+E` | Toggle mouse capture |
| `Esc` | Quit |

## Search Syntax

| Prefix | Example | Meaning |
|--------|---------|---------|
| `agent:` | `agent:claude` | Filter by agent |
| `-agent:` | `-agent:opencode` | Exclude agent |
| `dir:` | `dir:rust-resume` | Filter by directory |
| `date:` | `date:today`, `date:3d`, `date:1w` | Filter by time |

## Config

Optional config at `~/.config/rust-resume/config.toml`:

```toml
[agents.claude]
dir = "~/.claude/projects"

[agents.opencode]
db = "~/.local/share/opencode/opencode.db"
```

## License

MIT
