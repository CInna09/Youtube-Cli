package infra

import (
	"bufio"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
)

// CavaClient manages a cava(1) subprocess that outputs raw ASCII bar heights.
// Use the Bars channel to receive per-frame bar values (0-1000).
// If cava is not available or Start fails, Bars will never produce data
// and the caller should fall back to a synthetic visualiser.
type CavaClient struct {
	cmd    *exec.Cmd
	Bars   chan []int // receives bar heights; buffered (1) so sender never blocks
	mu     sync.Mutex
	running bool
}

// NewCavaClient creates a client. Call Start() to launch the process.
func NewCavaClient() *CavaClient {
	return &CavaClient{
		Bars: make(chan []int, 1),
	}
}

// Start launches cava with a dynamically-written raw-output config.
// Returns nil if cava was already running (idempotent).
// Returns an error if cava is not found or fails to start.
func (c *CavaClient) Start() error {
	c.mu.Lock()
	defer c.mu.Unlock()

	if c.running {
		return nil // idempotent
	}

	// Locate cava binary
	cavaBin, err := exec.LookPath("cava")
	if err != nil {
		return fmt.Errorf("cava not found: %w", err)
	}

	// Write a minimal raw-output config
	home, _ := os.UserHomeDir()
	confDir := home + "/.config/YTcliV2"
	confPath := filepath.Join(confDir, "cava.conf")
	if err := os.MkdirAll(confDir, 0755); err != nil {
		return fmt.Errorf("cava config dir: %w", err)
	}
	if err := os.WriteFile(confPath, []byte(cavaConfig), 0644); err != nil {
		return fmt.Errorf("write cava config: %w", err)
	}

	c.cmd = exec.Command(cavaBin, "-p", confPath)

	// Capture stdout for bar data
	stdout, err := c.cmd.StdoutPipe()
	if err != nil {
		return fmt.Errorf("cava stdout pipe: %w", err)
	}

	if err := c.cmd.Start(); err != nil {
		return fmt.Errorf("cava start: %w", err)
	}
	c.running = true

	// Start reader goroutine
	go c.readBars(stdout)

	return nil
}

// Stop kills the cava process. Safe to call multiple times.
func (c *CavaClient) Stop() {
	c.mu.Lock()
	defer c.mu.Unlock()

	if !c.running {
		return
	}
	c.running = false

	if c.cmd != nil && c.cmd.Process != nil {
		c.cmd.Process.Kill()
		c.cmd.Wait()
	}
}

// readBars reads ASCII frames from cava's stdout and pushes them into Bars.
func (c *CavaClient) readBars(stdout io.ReadCloser) {
	defer stdout.Close()

	sc := bufio.NewScanner(stdout)
	// Grow scanner buffer for long lines (20 bars × up to 4 digits + delimiters)
	sc.Buffer(make([]byte, 4096), 4096)

	for sc.Scan() {
		line := strings.TrimRight(sc.Text(), "\n\r")
		if line == "" {
			continue
		}
		parts := strings.Split(line, ";")
		bars := make([]int, 0, len(parts))
		for _, p := range parts {
			p = strings.TrimSpace(p)
			if p == "" {
				continue
			}
			v, err := strconv.Atoi(p)
			if err != nil {
				continue // skip malformed values
			}
			bars = append(bars, v)
		}
		if len(bars) == 0 {
			continue
		}

		// Non-blocking send — drop frame if nobody is reading
		select {
		case c.Bars <- bars:
		default:
		}
	}
}

// cavaConfig is the INI written to disk for raw ASCII output.
var cavaConfig = strings.TrimLeft(`
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

[color]
foreground = default
background = default
`, "\n")
