package service

import (
	"sync"
	"time"

	"YTcliV2/infra"
	"YTcliV2/model"
)

// PlayerService controls playback via mpv.
type PlayerService struct {
	mpv     *infra.MpvClient
	search  *SearchService
	cava    *infra.CavaClient
	config  *infra.Config
	track   *model.Track
	mu      sync.RWMutex
	stopCh  chan struct{}
	volume  int // cached volume, re-synced from mpv every 500ms via poll()
}

// Bars exposes per-frame bar heights from cava (0-1000 per bar, 20 bars).
// Listeners should use a non-blocking receive. The channel is buffered (1).
// Falls back to nil / no data if cava is unavailable.
func (s *PlayerService) Bars() chan []int {
	if s.cava == nil {
		return nil
	}
	return s.cava.Bars
}

// NewPlayerService creates a player service.
// cava may be nil; if so, audio visualisation is disabled.
// config is used to read/save volume preference.
func NewPlayerService(mpv *infra.MpvClient, search *SearchService, cava *infra.CavaClient, config *infra.Config) *PlayerService {
	p := &PlayerService{
		mpv:    mpv,
		search: search,
		cava:   cava,
		config: config,
		stopCh: make(chan struct{}),
		volume: config.DefaultVolume,
	}
	// Sync mpv to our persisted volume (hanya kalo mpv jalan)
	if config.DefaultVolume > 0 {
		mpv.SetVolume(config.DefaultVolume) // safe — sendCommand handles nil conn
	}
	return p
}

// Play starts streaming a video and launches the audio visualiser.
func (s *PlayerService) Play(v *model.Video) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	streamURL, err := s.search.StreamURL(v.ID)
	if err != nil {
		return err
	}

	s.track = &model.Track{
		Video:    v,
		Volume:   s.volume,
		Duration: 0,
	}

	if err := s.mpv.LoadURL(streamURL); err != nil {
		return err
	}
	s.mpv.SetVolume(s.volume)

	// Start cava visualiser (best-effort)
	if s.cava != nil {
		if cerr := s.cava.Start(); cerr != nil {
			// Non-fatal — visualiser stays disabled
		}
	}

	go s.poll()

	return nil
}

// poll periodically updates track state from mpv (position, duration, volume).
func (s *PlayerService) poll() {
	ticker := time.NewTicker(500 * time.Millisecond)
	defer ticker.Stop()
	for {
		select {
		case <-ticker.C:
			s.mu.Lock()
			pos, _ := s.mpv.GetPosition()
			dur, _ := s.mpv.GetDuration()
			vol, _ := s.mpv.GetVolume()
			if vol > 0 {
				s.volume = vol // sync dari mpv langsung
			}
			if s.track != nil {
				s.track.Position = pos
				s.track.Duration = dur
				s.track.Volume = s.volume
			}
			s.mu.Unlock()
		case <-s.stopCh:
			return
		}
	}
}

// Pause toggles pause on/off.
func (s *PlayerService) Pause() error {
	s.mu.Lock()
	defer s.mu.Unlock()

	if s.track == nil {
		return nil
	}
	paused := !s.track.Paused
	s.track.Paused = paused
	return s.mpv.SetPause(paused)
}

// Seek jumps forward/backward by the given seconds.
func (s *PlayerService) Seek(seconds float64) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.mpv.Seek(seconds)
}

// Stop stops playback and the audio visualiser.
func (s *PlayerService) Stop() error {
	s.mu.Lock()
	defer s.mu.Unlock()

	close(s.stopCh)
	s.stopCh = make(chan struct{})
	s.track = nil

	if s.cava != nil {
		s.cava.Stop()
	}
	return s.mpv.Stop()
}

// SetVolume sets volume 0-100. Writes to mpv immediately and saves to config.
// Cache is updated at the same time, then re-synced from mpv every 500ms via poll().
func (s *PlayerService) SetVolume(level int) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	if level < 0 {
		level = 0
	}
	if level > 100 {
		level = 100
	}
	s.volume = level
	if s.track != nil {
		s.track.Volume = level
	}
	// Persist ke config biar inget next session
	s.config.DefaultVolume = level
	s.config.Save() // best-effort

	return s.mpv.SetVolume(level)
}

// Volume returns the current persisted volume level.
func (s *PlayerService) Volume() int {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.volume
}

// State returns a copy of the current track state.
func (s *PlayerService) State() *model.Track {
	s.mu.RLock()
	defer s.mu.RUnlock()

	if s.track == nil {
		return nil
	}
	cp := *s.track
	if cp.Video != nil {
		v := *cp.Video
		cp.Video = &v
	}
	return &cp
}
