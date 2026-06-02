package service

import (
	"sync"

	"YTcliV2/infra"
	"YTcliV2/model"
)

// SearchService queries for videos.
type SearchService struct {
	client *infra.YtdlpClient
	mu     sync.Mutex
}

// NewSearchService creates a search service.
func NewSearchService(client *infra.YtdlpClient) *SearchService {
	return &SearchService{client: client}
}

// Search returns videos matching query.
func (s *SearchService) Search(query string, limit int) ([]*model.Video, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.client.Search(query, limit)
}

// StreamURL resolves the stream URL for a video ID.
func (s *SearchService) StreamURL(videoID string) (string, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.client.StreamURL(videoID)
}
