<div align="center">
  <img src="images/tflow_logo.png" alt="tFlow Logo" width="200"/>
  <h1>tFlow (Beta) </h1>
  <p><strong>Terminal-native text & markdown editor</strong></p>
  <p>
    <img src="images/showcase.png" alt="tFlow Showcase" width="90%"/>
  </p>
</div>

---

## Features

- **Dual preview** — Plain text view for `.txt` files, styled markdown preview for `.md` files
- **Rope-based engine** — Handles 100k+ line files with low latency
- **Vim-inspired modes** — Normal, Insert, Visual, Command, Search
- **Markdown rendering** — Headings, tables, lists, code blocks, blockquotes, checkboxes
- **Workspace tools** — File tree browser, workspace grep search
- **Full undo/redo** — With change-grouping for natural history
- **Clipboard** — System clipboard integration (copy, cut, paste)
- **Multi-buffer** — Tab through open files, split panes
- **Command palette** — Fuzzy-find any command
- **Configurable** — TOML config at `~/.config/tflow/config.toml`
- **Themes** — retro_green, amber, synthwave, tokyo_night, default_dark

---

## Install

### From source

```bash
git clone https://github.com/Simangka/tFlow.git
cd tFlow
cargo build --release
cp target/release/tflow ~/.local/bin/
```

Or run directly:

```bash
cargo run -- notes.md
```

### Requirements

- Rust 1.75+
- A truecolor terminal (Windows Terminal, Kitty, Alacritty, iTerm2, GNOME Terminal)

---

## Usage

### Opening files

```bash
tflow notes.md              # open a file
tflow a.md b.txt            # open multiple files
tflow .                     # open current directory as workspace
tflow notes.md:120          # open at line 120
cat notes.md | tflow        # pipe from stdin
```

### Modes

tFlow uses a vim-inspired modal system:

| Mode | Enter | Purpose |
|------|-------|---------|
| **Normal** | `Esc` | Navigate and manipulate text |
| **Insert** | `i` | Type and edit text |
| **Visual** | `v` | Select text |
| **Command** | `:` | Run commands |
| **Search** | `/` | Find text |

---

### Keybindings

#### Normal mode

| Key | Action |
|-----|--------|
| `h` `j` `k` `l` | Move cursor |
| `w` `b` | Word forward / backward |
| `0` `$` | Start / end of line |
| `gg` `G` | Start / end of file |
| `%` | Matching brace |
| `Ctrl+u` `Ctrl+d` | Half page up / down |
| `Ctrl+b` `Ctrl+f` | Page up / down |
| `i` `a` | Insert mode (before / after cursor) |
| `o` `O` | Insert new line (below / above) |
| `v` `V` | Visual / Visual line mode |
| `x` | Delete character |
| `dd` | Delete line |
| `yy` | Copy (yank) line |
| `p` `P` | Paste |
| `u` | Undo |
| `Ctrl+r` | Redo |
| `>` `<` | Indent / Unindent |
| `D` | Delete to end of line |
| `J` | Join lines |

#### Command mode

| Command | Action |
|---------|--------|
| `:w` | Save file |
| `:q` | Quit |
| `:wq` | Save and quit |
| `:q!` | Force quit |
| `:e <file>` | Open file |
| `:new` | New buffer |
| `:help` | Show help |

#### Global

| Key | Action |
|-----|--------|
| `Ctrl+s` | Save |
| `Ctrl+q` | Quit |
| `Ctrl+p` | Command palette |
| `Ctrl+k` / `F11` | Toggle markdown preview |
| `Alt+m` | Toggle plain / markdown preview mode |
| `F1` | Toggle file tree |
| `Tab` / `Shift+Tab` | Next / previous buffer |

#### Search mode

| Key | Action |
|-----|--------|
| `/` | Search forward |
| `?` | Search backward |
| `n` | Next match |
| `N` | Previous match |
| `Enter` | Execute search |
| `Esc` | Cancel search |

---

### Configuration

Config file at `~/.config/tflow/config.toml`:

```toml
theme = "retro_green"

[line_numbers]
enabled = true
relative = false

[editor]
tab_width = 4
autosave = true
cursor_blink_period_ms = 500
scrolloff = 3
history_size = 1000

[markdown]
preview_width_ratio = 0.5
live_preview = true

[ui]
show_statusbar = true
show_commandbar = true
```

#### Themes

- `retro_green` — Phosphor green CRT aesthetic
- `amber` — Warm amber terminal
- `synthwave` — Purple/cyan synthwave
- `tokyo_night` — Blue-based Tokyo Night
- `default_dark` — Modern dark theme

---

### Architecture

```
src/
  app/           Application state, event loop
  core/          Text buffer (ropey), position, range types
  ui/            Layout, statusline, panels, widgets
  input/         Crossterm event stream
  commands/      Action enum, keymap, command registry, palette
  editor/        Cursor, selection, modes, history, edit operations
  markdown/      Parser and renderer (plain text + markdown)
  rendering/     Dirty-region render engine, line numbers, scrollbar
  theme/         Color schemes, syntax highlighting
  config/        TOML config loading
  workspace/     File tree, grep search
  async_tasks/   Background task queue (autosave, file watching)
  plugins/       Plugin system architecture (future WASM support)
```

---

## Built with Rust


---
