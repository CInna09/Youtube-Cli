package infra

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os/exec"
	"strconv"
	"strings"

	"YTcliV2/model"
)

// YtdlpClient wraps the yt-dlp binary.
type YtdlpClient struct {
	Binary string // default: "yt-dlp"
}

// NewYtdlpClient creates a default client.
func NewYtdlpClient() *YtdlpClient {
	return &YtdlpClient{Binary: "yt-dlp"}
}

// ytdlpResult maps a single JSON line from yt-dlp --dump-json.
type ytdlpResult struct {
	ID          string  `json:"id"`
	Title       string  `json:"title"`
	Channel     string  `json:"channel"`
	Duration    float64 `json:"duration"`
	ViewCount   int64   `json:"view_count"`
	LiveStatus  string  `json:"live_status"`
	WebpageURL  string  `json:"webpage_url"`
}

// Search runs yt-dlp ytsearch and returns parsed results.
func (c *YtdlpClient) Search(query string, limit int) ([]*model.Video, error) {
	searchStr := fmt.Sprintf("ytsearch%d:%s", limit, query)
	args := []string{
		searchStr,
		"--dump-json",
		"--no-download",
		"--flat-playlist",
		"--ignore-errors",
	}

	cmd := exec.Command(c.Binary, args...)
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr

	if err := cmd.Run(); err != nil {
		return nil, fmt.Errorf("yt-dlp search: %w\n%s", err, stderr.String())
	}

	var videos []*model.Video
	lines := strings.Split(strings.TrimSpace(stdout.String()), "\n")
	for i, line := range lines {
		if line == "" {
			continue
		}
		var raw ytdlpResult
		if err := json.Unmarshal([]byte(line), &raw); err != nil {
			continue
		}
		videos = append(videos, &model.Video{
			Index:    i + 1,
			ID:       raw.ID,
			Title:    raw.Title,
			Channel:  raw.Channel,
			Duration: secondsToTimeStr(int(raw.Duration)),
			Views:    formatCount(raw.ViewCount),
			IsLive:   raw.LiveStatus == "is_live" || raw.LiveStatus == "is_upcoming",
			URL:      raw.WebpageURL,
		})
	}
	return videos, nil
}

// StreamURL resolves the best audio stream URL for a video ID.
func (c *YtdlpClient) StreamURL(videoID string) (string, error) {
	url := "https://youtube.com/watch?v=" + videoID
	cmd := exec.Command(c.Binary,
		"-f", "bestaudio",
		"-g",
		"--no-download",
		"--ignore-errors",
		url,
	)
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr

	if err := cmd.Run(); err != nil {
		return "", fmt.Errorf("yt-dlp stream: %w\n%s", err, stderr.String())
	}

	streamURL := strings.TrimSpace(stdout.String())
	if streamURL == "" {
		return "", fmt.Errorf("yt-dlp returned empty stream URL")
	}
	return streamURL, nil
}

// secondsToTimeStr converts 1110 to "18:30".
func secondsToTimeStr(s int) string {
	h := s / 3600
	m := (s % 3600) / 60
	sec := s % 60
	if h > 0 {
		return fmt.Sprintf("%d:%02d:%02d", h, m, sec)
	}
	return fmt.Sprintf("%d:%02d", m, sec)
}

// formatCount converts 1234567 to "1.2m".
func formatCount(n int64) string {
	switch {
	case n >= 1_000_000_000:
		return fmt.Sprintf("%.1fb", float64(n)/1_000_000_000)
	case n >= 1_000_000:
		return fmt.Sprintf("%.1fm", float64(n)/1_000_000)
	case n >= 1_000:
		return fmt.Sprintf("%.1fk", float64(n)/1_000)
	default:
		return strconv.FormatInt(n, 10)
	}
}
