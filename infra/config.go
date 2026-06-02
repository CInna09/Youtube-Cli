package infra

import (
	"os"
	"path/filepath"

	"github.com/BurntSushi/toml"
)

// Config holds application settings.
type Config struct {
	SocketPath    string   `toml:"socket_path"`    // mpv IPC socket
	DefaultVolume int      `toml:"default_volume"` // persisted volume (0-100)
	ExtraArgs     []string `toml:"extra_args"`     // extra args passed to yt-dlp (e.g. ["--auth-from-browser"])
}

// DefaultConfig returns a Config with sensible defaults.
func DefaultConfig() *Config {
	return &Config{
		SocketPath:    "/tmp/YTcliV2-mpv.sock",
		DefaultVolume: 30,
	}
}

// configPath returns the expected config file location.
func configPath() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(home, ".config", "YTcliV2", "config.toml"), nil
}

// Load reads a Config from disk or returns defaults.
func Load() (*Config, error) {
	cfg := DefaultConfig()
	path, err := configPath()
	if err != nil {
		return cfg, err
	}

	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return cfg, nil
		}
		return cfg, err
	}

	if err := toml.Unmarshal(data, cfg); err != nil {
		return cfg, err
	}
	return cfg, nil
}

// Save persists the Config to disk.
func (c *Config) Save() error {
	path, err := configPath()
	if err != nil {
		return err
	}

	dir := filepath.Dir(path)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return err
	}

	f, err := os.Create(path)
	if err != nil {
		return err
	}
	defer f.Close()

	return toml.NewEncoder(f).Encode(c)
}
