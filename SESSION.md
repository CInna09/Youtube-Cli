# SESSION MEMORY — YTcliV2

## Timestamp
2026-06-02 — Ongoing session

---

## Project Overview
**YTcliV2** — Single-binary YouTube CLI with search, audio/video playback via mpv, and real-time audio-driven equalizer visualization via cava.

---

## Architecture

### Module Tree
```
YTcliV2/
├── main.go              # Entry point, service wiring
├── model/
│   └── video.go         # Video, Playlist, Track structs
├── infra/
│   ├── config.go        # TOML config ~/.config/YTcliV2/config.toml
│   ├── ytdlp.go         # YtdlpClient (Search, StreamURL)
│   ├── mpv.go           # MpvClient (Unix socket JSON-RPC)
│   └── cava.go          # CavaClient (subprocess, raw ASCII bar reader)
├── service/
│   ├── search.go        # SearchService wrapping ytdlp
│   ├── player.go        # PlayerService (mpv lifecylce + cava lifecycle)
│   └── cache.go         # JSON history cache (max 50 entries)
└── ui/
    ├── app.go           # bubbletea model (struct, New, Init, Update, handleKey)
    ├── layout.go        # View() + all render functions
    ├── keys.go          # KeyMap + DefaultKeyMap (bubbles/key library)
    └── styles.go        # lipgloss styles (NormalBorder, teal palette)
```

### Data Flow
```
User Input → tea.KeyMsg → handleKey() → service calls
                                               ↓
yt-dlp subprocess ──→ SearchService ──→ results []*Video
                                               ↓
mpv (JSON IPC socket) ←── PlayerService ──→ Track state
                                               ↓
cava (raw ASCII stdout) ←── CavaClient ──→ Bars chan []int
                                               ↓
bubbletea tick (500ms) → read Bars non-blocking → eqBars []int
                                               ↓
renderEqBars(count, paused, eqBars) → block chars ▁▂▃▄▅▆▇█
```

---

## File Details

### `main.go`
- Loads TOML config (SocketPath, etc.)
- Creates infra: ytdlp, mpv, cava
- Creates services: search, player, cache
- Launches bubbletea program with alt screen
- On exit: mpv.Quit()
- **cava**: created via `infra.NewCavaClient()`, passed to `service.NewPlayerService(mpv, searchSvc, cava)`

### `infra/cava.go` — CavaClient (NEW)
```go
type CavaClient struct {
    cmd     *exec.Cmd
    Bars    chan []int   // buffered(1) — real bar heights 0-1000
    done    chan struct{}
    mu      sync.Mutex
    running bool
}
```
**Config written to disk** (`~/.config/YTcliV2/cava.conf`):
```ini
[general]
bars = 20
framerate = 30

[input]
method = pulse
source = auto

[output]
method = raw
raw_target = /dev/stdout
data_format = ascii
ascii_max_range = 1000
bar_delimiter = 59
frame_delimiter = 10
```
- `Start()`: idempotent, writes config, spawns `cava -p <config>`, starts reader goroutine
- `Stop()`: kills process via `Process.Kill()` + `Wait()`
- `readBars()`: bufio.Scanner on stdout, splits by `;`, parses ints
- Non-blocking send on `Bars` chan (drops frame if reader busy)
- Trailing semicolon in cava output handled by `TrimSpace` + empty-skip
- cava binary at `/usr/sbin/cava` — `exec.LookPath("cava")` succeeds

### `service/player.go` — PlayerService
```go
type PlayerService struct {
    mpv     *infra.MpvClient
    search  *SearchService
    cava    *infra.CavaClient    // may be nil
    config  *infra.Config        // for volume persistence
    track   *model.Track
    mu      sync.RWMutex
    stopCh  chan struct{}
    volume  int                  // persisted across track changes
}
```
- `Bars()` returns `s.cava.Bars` (or nil if cava is nil)
- `Play()` → starts mpv playback + calls `s.cava.Start()` (best-effort, error non-fatal). Uses `s.volume` instead of hardcoded 60.
- `Stop()` → kills cava + mpv
- `SetVolume(level)` → persists to `s.volume`, saves `config.DefaultVolume`, best-effort `config.Save()`
- `Volume()` getter → returns current persisted volume
- `NewPlayerService(mpv, search, cava, config)` → reads initial volume from `config.DefaultVolume`
- Poll goroutine: every 500ms reads mpv position/duration

### `ui/app.go` — bubbletea Model

**Struct:**
```go
type App struct {
    width, height int
    state         viewState   // stateSearch / statePlayer
    search        textinput.Model
    results       []*model.Video
    cursor        int
    track         *model.Track
    volume        int
    tick          uint64      // sine wave counter (also used even with cava)
    eqBars        []int       // latest cava bars (nil = use synth fallback)
    searchSvc     *service.SearchService
    playerSvc     *service.PlayerService
    cacheSvc      *service.CacheService
    fetching      bool
    err           error
}
```

**KeyMsg handling:**
- `/` → toggle search focus
- `Enter` (focused) → search
- `Enter` (not focused) → play audio (`bestaudio`)
- `v` → play video (`best`)
- `Space`/`p` → play/pause toggle
- `s`/`F4` → stop
- `←`/`→` → seek ±5s
- `+`/`=`/`F3` → volume up 5
- `-`/`F2` → volume down 5
- `F1` → mute/unmute toggle
- `Ctrl+C`/`q` → quit
- `↑`/`k`/`↓`/`j` → cursor navigation

**playerTickMsg handler:**
1. Reads track state from PlayerService
2. Non-blocking receive from `playerSvc.Bars()` → `eqBars`
3. Increments `tick` (used for animation cadence even with real bars)
4. Re-polls via `pollPlayerTick()`

### `ui/layout.go` — Rendering

**View() layout:**
```
┌──────────────────────────────────────────┐
│  sidebar (20)  │  main content           │
│  logo          │  header + search bar    │
│  keybinds      │  table (dynamic cols)   │
│  volume bar    │                         │
├────────────────┴─────────────────────────┤
│  [eq left] Title | time/pos | Vol [eq right]  │
└──────────────────────────────────────────┘
```

**renderStatusContent():**
- Info text: `Title | MM:SS / MM:SS [PAUSED] | Vol: N%`
- Calculates remaining space for eq bars (even split left/right)
- Passes `a.eqBars` to `renderEqBars()`

**renderEqBars() — two modes:**

1. **Real cava data** (len(bars) > 0):
   - Maps `count` display chars across `len(bars)` bars via `i * len(bars) / count`
   - Scales 0-1000 → 0-7 block index: `bars[idx] * 7 / 1000`
   - Thrshold: index >= 3 → volFilledStyle (teal), else dimStyle (gray)
   - Paused: all bars show flat `▁` in dim

2. **Synthetic fallback** (bars empty/nil):
   - Sine wave: `sin(tick*0.08 + i*0.4 + phaseOffset)` → 0-7
   - 3× slower animation (0.08 vs original 0.25)
   - Left/right halves have offset phase for variety

### `ui/keys.go` — KeyMap
Uses `bubbles/key` library with `key.Matches()` instead of raw string comparisons.
```go
type KeyMap struct {
    Quit, FocusSearch, CursorUp, CursorDown, Select,
    PlayVideo, PlayPause, Stop, SeekBack, SeekForward,
    VolumeUp, VolumeDown, Mute key.Binding
}
```

### `ui/styles.go` — lipgloss Styles
- NormalBorder (┌─┐ └─┘)
- Colors: Teal `#00B894`, Dim `#4A5568`, Text `#E2E8F0`, Muted `#718096`, Highlight `#00CEC9`, Live `#FF6B6B`, Bg `#0D1117`
- panelStyle, logoStyle, headerStyle, tableHeaderStyle, rowStyle, selectedRowStyle, statusBarStyle, volFilledStyle, volEmptyStyle, dimStyle, errorStyle, liveBadgeStyle

### `infra/mpv.go` — MpvClient
Unix socket JSON-RPC to mpv:
- `Start()`: spawns `mpv --idle --no-terminal --no-video --input-ipc-server=<socket>`
- `LoadURL(url)`, `SetPause(bool)`, `SetVolume(int)`, `GetPosition()`, `GetDuration()`, `Seek(seconds)`, `Stop()`, `Quit()`
- Wait loop up to 5s for socket to appear
- JSON-RPC with request/response pairs

### `infra/ytdlp.go` — YtdlpClient
- `Search(query, limit)` → `[]*model.Video` parsing yt-dlp JSON output
- `StreamURL(videoID)` → extracts best audio/video stream URL via yt-dlp
- Uses `--flat-playlist` for search, `--get-url` for stream extraction

---

## Build & Deployment

### Build Command
```bash
CGO_ENABLED=0 go build -ldflags="-s -w" -o YTcliV2 .
```
Produces 4.1MB static ELF binary.

### System Dependencies
- **mpv** at `/usr/sbin/mpv` v0.41.0 — media player (JSON IPC)
- **yt-dlp** at `/usr/bin/yt-dlp` and `/usr/sbin/yt-dlp` — YouTube extraction
- **cava** at `/usr/sbin/cava` — audio visualizer (PulseAudio FFT)
- **PulseAudio** — audio server (`pactl info` confirms running)

### Runtime Files
- `~/.config/YTcliV2/config.toml` — user config (socket_path, default_volume)
- `~/.config/YTcliV2/cache.json` — search history (max 50 entries)
- `~/.config/YTcliV2/cava.conf` — cava raw output config (auto-generated)
- `/tmp/YTcliV2-mpv.sock` — mpv IPC socket

---

## Key Design Decisions

1. **Single outer panel** for sidebar+main instead of two separate panels: eliminates gap and double-border between them
2. **bubbles/key** for key bindings: enables `key.Matches()` instead of fragile string switches
3. **Dynamic table columns**: title 55%, channel 25% of available space (not fixed 42/18)
4. **Equalizer repeats to fill width**: renderEqBars generates N single-char block bars on both sides of info text, statusBarStyle.Width(width) ensures no gap before right border
5. **50/50 left/right eq split**: equal visual weight
6. **Cava lifecycle bound to PlayerService**: auto-starts on Play(), auto-stops on Stop()
7. **Non-blocking bar reads**: cava writer never blocks; dropped frames are invisible
8. **Graceful degradation**: if cava not installed or fails, falls back to sine wave animation
9. **Best-effort cava start**: Start() errors are non-fatal, playback continues without visualization
10. **Search blur on results**: search.Blur() after results arrive so next Enter plays instead of re-searching
11. **Volume persisted to config**: PlayerService stores volume in `Config.DefaultVolume` and saves to `config.toml` on every change. Survives app restarts.
12. **Volume synced from mpv every 500ms via poll()**: PlayerService.poll() calls mpv.GetVolume() and updates `s.volume` + `track.Volume`. This is the "ambil dari mesin langsung" approach — display always shows the actual mpv volume, not a locally-tracked copy.
13. **No hardcoded Volume in Play()**: `Play()` uses `s.volume` (synced from mpv) instead of hardcoding 60.
14. **tea.Tick(100ms) for UI poll**: prevents 100% CPU spin from immediate recursive command. 100ms = 10fps, smooth enough for eq animation without wasting CPU.
15. **tea.Cmd instead of raw goroutine for play**: `playSelected` returns `tea.Batch(playCmd, pollPlayerTick)` — avoids data race on `a.err` and keeps bubbletea's event-loop safety.
16. **Global action keys before focus check**: VolumeUp/Down, Mute, Stop, PlayPause, SeekBack/Forward are matched BEFORE the `a.search.Focused()` check. This means F1/F2/F3 work even when typing in the search box — no more "volume mockup" where keys get eaten by search input.
17. **Mute uses mpv.Volume() not a.volume**: Mute toggle checks `playerSvc.Volume()` which reads the actual mpv state, not the local `a.volume` which could be stale. Also saves pre-mute volume in `a.mutedVolume` for proper restore on unmute.
18. **Volume bar width=12 + percentage text**: Changed from width=8 (where 30%→35% was invisible) to width=12 plus `fmt.Sprintf("%d%%", vol)` so changes are always visible.
19. **MpvClient mutex**: Added `sync.Mutex` to MpvClient to protect the JSON-RPC connection, making it safe for concurrent use from poll() goroutine and UI goroutine.

---

## cava Integration Details

### Configuration
The cava config file is written dynamically to `~/.config/YTcliV2/cava.conf`:
```ini
[general]
bars = 20        # number of FFT bars
framerate = 30   # frames per second output

[input]
method = pulse   # capture from PulseAudio
source = auto    # monitor of default sink

[output]
method = raw                     # raw data stream (not curses)
raw_target = /dev/stdout         # output to stdout
data_format = ascii              # human-readable numbers
ascii_max_range = 1000           # values 0-1000
bar_delimiter = 59               # semicolon between bars
frame_delimiter = 10             # newline between frames
```

### Output Format (confirmed working)
```
350;420;180;90;50;30;20;15;12;10;8;6;5;4;3;3;2;2;1;1;
```
- 20 semicolon-delimited integers per line
- Each integer 0-1000
- Trailing semicolon after last value (handled by parser)
- One line per frame at 30 fps

### Parser (cava.go:readBars)
```go
sc.Buffer(make([]byte, 4096), 4096)  // grow for long lines
sc.Scan()                            // read lines
parts := strings.Split(line, ";")    // split by semicolon
for _, p := range parts {
    p = strings.TrimSpace(p)
    if p == "" { continue }
    v, _ := strconv.Atoi(p)
    bars = append(bars, v)
}
select {
case c.Bars <- bars:   // send to UI
default:               // drop if UI not reading
}
```

### Lifecycle
- **Start**: Called from PlayerService.Play() when a track starts
- **Running**: Idempotent — cava continues across track changes (play→play)
- **Pause**: mpv pauses audio → PulseAudio monitor goes silent → cava outputs all 0s → UI shows flat `▁` (correct behavior)
- **Stop**: cava.Stop() kills process, cava.running = false
- **Quit**: If track still playing, mpv.Quit() doesn't stop cava — but the binary exits so OS cleans up

### Why Not Real FFT in Go?
cava is a mature, optimized C implementation using PulseAudio/PipeWire APIs for audio capture and FFTW for transforms. Reimplementing that in Go would add complexity, latency, and maintenance burden. Using cava as a subprocess gives us:
- Battle-tested audio capture (PulseAudio monitor source)
- Hardware-accelerated FFT
- Built-in smoothing/falloff (monstercat smoothing)
- No dependency on CGO
- ~20 lines of Go glue code

---

## Terminal Environment
- **Terminal**: kitty (likely)
- **Font**: Supports block chars ▁▂▃▄▅▆▇█ (full Unicode range)
- **F-keys**: ThinkPad X240 requires FnLock (Fn+Esc) for F1-F4 to work as function keys instead of media keys

---

## Session Commands Reference
```bash
# Build
cd /home/scvi/AI/gabut
CGO_ENABLED=0 go build -ldflags="-s -w" -o YTcliV2 .

# Test cava config
mkdir -p /tmp/cava-test
cava -p /tmp/cava-test/cava.conf

# Verify cava raw output (runs 2 seconds)
timeout 2 cava -p /path/to/cava.conf | head -5

# Check mpv
mpv --version

# Check yt-dlp
yt-dlp --version

# Check audio system
pactl info
```
