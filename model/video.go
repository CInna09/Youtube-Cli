package model

// Video represents a single YouTube video entry.
type Video struct {
	Index    int
	ID       string
	Title    string
	Channel  string
	Duration string // e.g. "18:22"
	Views    string // e.g. "1.2m"
	IsLive   bool
	URL      string
}

// Playlist is an ordered list of videos queued for playback.
type Playlist struct {
	Items   []*Video
	Current int
}

// Track holds the current playback state.
type Track struct {
	Video    *Video
	Position float64 // seconds
	Duration float64 // seconds
	Volume   int     // 0-100
	Paused   bool
}
