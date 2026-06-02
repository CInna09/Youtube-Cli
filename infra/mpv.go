package infra

import (
	"bufio"
	"encoding/json"
	"fmt"
	"net"
	"os"
	"os/exec"
	"sync"
	"time"
)

// MpvClient controls a running mpv instance via its JSON IPC socket.
// All methods are safe for concurrent use.
type MpvClient struct {
	SocketPath string // e.g. /tmp/YTcliV2-mpv.sock
	Binary     string // default: "mpv"

	mu      sync.Mutex
	conn    net.Conn
	reader  *bufio.Scanner
	cmd     *exec.Cmd
	running bool
}

// NewMpvClient creates a client for an mpv instance.
func NewMpvClient(socketPath, binary string) *MpvClient {
	return &MpvClient{
		SocketPath: socketPath,
		Binary:     binary,
	}
}

// mpvRequest is a JSON-RPC command.
type mpvRequest struct {
	Command   []interface{} `json:"command"`
	RequestID int           `json:"request_id,omitempty"`
}

// mpvResponse is a JSON-RPC response.
type mpvResponse struct {
	Error     string      `json:"error"`
	Data      interface{} `json:"data"`
	RequestID int         `json:"request_id"`
}

// Start spawns mpv with --input-ipc-server and waits for the socket.
func (m *MpvClient) Start() error {
	os.Remove(m.SocketPath)

	m.cmd = exec.Command(m.Binary,
		"--idle",
		"--no-terminal",
		"--no-video",
		"--no-audio-display",
		"--volume=100",
		"--volume-max=100",
		"--replaygain=no",
		"--af=scaletempo",
		fmt.Sprintf("--input-ipc-server=%s", m.SocketPath),
	)
	if err := m.cmd.Start(); err != nil {
		return fmt.Errorf("mpv exec: %w", err)
	}
	m.running = true

	// Wait for socket to appear
	for i := 0; i < 50; i++ {
		if _, err := os.Stat(m.SocketPath); err == nil {
			return m.connect()
		}
		time.Sleep(100 * time.Millisecond)
	}
	return fmt.Errorf("mpv socket not ready after 5s")
}

// connect opens the Unix socket connection.
func (m *MpvClient) connect() error {
	conn, err := net.Dial("unix", m.SocketPath)
	if err != nil {
		return fmt.Errorf("mpv dial: %w", err)
	}
	m.conn = conn
	m.reader = bufio.NewScanner(conn)
	return nil
}

// sendCommand sends a JSON command and reads one response line.
// Returns an error if mpv is not connected (nil-conn safe).
func (m *MpvClient) sendCommand(cmd []interface{}) (*mpvResponse, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	if m.conn == nil {
		return nil, fmt.Errorf("mpv not connected")
	}

	req := mpvRequest{Command: cmd, RequestID: 1}
	data, err := json.Marshal(req)
	if err != nil {
		return nil, fmt.Errorf("mpv marshal: %w", err)
	}
	if _, err := m.conn.Write(append(data, '\n')); err != nil {
		return nil, fmt.Errorf("mpv write: %w", err)
	}
	if m.reader.Scan() {
		var resp mpvResponse
		if err := json.Unmarshal([]byte(m.reader.Text()), &resp); err != nil {
			return nil, fmt.Errorf("mpv unmarshal: %w", err)
		}
		if resp.Error != "success" && resp.Error != "" {
			return &resp, fmt.Errorf("mpv error: %s", resp.Error)
		}
		return &resp, nil
	}
	return nil, fmt.Errorf("mpv no response")
}

// LoadURL tells mpv to load a stream URL.
func (m *MpvClient) LoadURL(url string) error {
	_, err := m.sendCommand([]interface{}{"loadfile", url, "replace"})
	return err
}

// SetPause pauses or resumes.
func (m *MpvClient) SetPause(paused bool) error {
	_, err := m.sendCommand([]interface{}{"set_property", "pause", paused})
	return err
}

// SetVolume sets volume 0-100.
func (m *MpvClient) SetVolume(level int) error {
	_, err := m.sendCommand([]interface{}{"set_property", "volume", level})
	return err
}

// GetVolume returns the current mpv volume (0-100).
func (m *MpvClient) GetVolume() (int, error) {
	resp, err := m.sendCommand([]interface{}{"get_property", "volume"})
	if err != nil {
		return 0, err
	}
	switch v := resp.Data.(type) {
	case float64:
		return int(v), nil
	}
	return 0, nil
}

// GetPosition returns current position in seconds.
func (m *MpvClient) GetPosition() (float64, error) {
	resp, err := m.sendCommand([]interface{}{"get_property", "time-pos"})
	if err != nil {
		return 0, err
	}
	switch v := resp.Data.(type) {
	case float64:
		return v, nil
	}
	return 0, nil
}

// Seek jumps forward (+seconds) or backward (-seconds).
func (m *MpvClient) Seek(seconds float64) error {
	_, err := m.sendCommand([]interface{}{"seek", seconds, "relative"})
	return err
}

// Stop stops current playback.
func (m *MpvClient) Stop() error {
	_, err := m.sendCommand([]interface{}{"stop"})
	return err
}

// GetDuration returns total duration in seconds.
func (m *MpvClient) GetDuration() (float64, error) {
	resp, err := m.sendCommand([]interface{}{"get_property", "duration"})
	if err != nil {
		return 0, err
	}
	switch v := resp.Data.(type) {
	case float64:
		return v, nil
	}
	return 0, nil
}

// Quit shuts down mpv.
func (m *MpvClient) Quit() error {
	m.sendCommand([]interface{}{"quit"})
	if m.conn != nil {
		m.conn.Close()
	}
	if m.cmd != nil && m.cmd.Process != nil {
		m.cmd.Process.Kill()
		m.cmd.Wait()
	}
	os.Remove(m.SocketPath)
	m.running = false
	return nil
}
