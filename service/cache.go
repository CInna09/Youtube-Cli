package service

import (
	"encoding/json"
	"os"
	"sync"

	"YTcliV2/model"
)

// CacheService stores history and bookmarks on disk.
type CacheService struct {
	filePath string
	mu       sync.RWMutex
	history  []*model.Video
}

const maxHistory = 50

// NewCacheService creates a cache service.
func NewCacheService(configDir string) *CacheService {
	return &CacheService{
		filePath: configDir + "/cache.json",
		history:  make([]*model.Video, 0, maxHistory),
	}
}

// Load reads history from disk.
func (c *CacheService) Load() error {
	c.mu.Lock()
	defer c.mu.Unlock()

	data, err := os.ReadFile(c.filePath)
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return err
	}
	return json.Unmarshal(data, &c.history)
}

// Save writes history to disk.
func (c *CacheService) Save() error {
	c.mu.RLock()
	defer c.mu.RUnlock()

	data, err := json.MarshalIndent(c.history, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(c.filePath, data, 0644)
}

// AddHistory appends a video to the history list.
func (c *CacheService) AddHistory(v *model.Video) error {
	c.mu.Lock()
	defer c.mu.Unlock()

	for i, h := range c.history {
		if h.ID == v.ID {
			c.history = append(c.history[:i], c.history[i+1:]...)
			break
		}
	}
	c.history = append([]*model.Video{v}, c.history...)
	if len(c.history) > maxHistory {
		c.history = c.history[:maxHistory]
	}
	return nil
}

// History returns all cached videos.
func (c *CacheService) History() ([]*model.Video, error) {
	c.mu.RLock()
	defer c.mu.RUnlock()

	result := make([]*model.Video, len(c.history))
	copy(result, c.history)
	return result, nil
}

// Bookmark marks a video (for v1 just adds to history).
func (c *CacheService) Bookmark(v *model.Video) error {
	return c.AddHistory(v)
}
