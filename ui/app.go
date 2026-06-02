package ui

import (
	"time"

	"github.com/charmbracelet/bubbles/key"
	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"

	"YTcliV2/model"
	"YTcliV2/service"
)

// viewState enum
type viewState int

const (
	stateSearch viewState = iota // showing search results
	statePlayer                  // video is playing
)

const searchResultLimit = 12

// Messages
type (
	searchDoneMsg struct {
		results []*model.Video
		err     error
	}
	playerTickMsg  struct{}
	playDoneMsg    struct {
		vid *model.Video
		err error
	}
)

// App is the root bubbletea model.
type App struct {
	width  int
	height int

	state   viewState
	search  textinput.Model
	results []*model.Video
	cursor  int

	track       *model.Track
	volume      int      // di-sync dari mpv tiap tick via playerSvc.Volume()
	mutedVolume int      // volume sebelum mute (buat unmute restore)
	tick        uint64   // counter for equalizer animation
	eqBars      []int    // latest bar heights from cava (0-1000 each), nil=use synth

	// Services (injected)
	searchSvc *service.SearchService
	playerSvc *service.PlayerService
	cacheSvc  *service.CacheService

	fetching bool
	err      error
}

// New creates the initial App model with service wiring.
func New(searchSvc *service.SearchService, playerSvc *service.PlayerService, cacheSvc *service.CacheService) *App {
	ti := textinput.New()
	ti.Placeholder = "Search YouTube..."
	ti.Focus()
	ti.CharLimit = 120

	ti.Width = 60 // sensible default; overridden by WindowSizeMsg

	return &App{
		search:    ti,
		results:   make([]*model.Video, 0),
		volume:    playerSvc.Volume(),
		searchSvc: searchSvc,
		playerSvc: playerSvc,
		cacheSvc:  cacheSvc,
	}
}

// -- bubbletea interface --

func (a *App) Init() tea.Cmd {
	return textinput.Blink
}

func (a *App) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {

	case tea.WindowSizeMsg:
		a.width = msg.Width
		a.height = msg.Height
		// Search input width: terminal - outerPanel(2) - sidebar(20) - separator(1) - padding(1)
		if w := msg.Width - 24; w > 10 {
			a.search.Width = w
		}

	case tea.KeyMsg:
		return a.handleKey(msg)

	case searchDoneMsg:
		a.fetching = false
		if msg.err != nil {
			a.err = msg.err
			return a, nil
		}
		a.results = msg.results
		a.cursor = 0
		a.search.Blur() // unfocus so next Enter plays selection
		return a, nil

	case playerTickMsg:
		t := a.playerSvc.State()
		if t != nil {
			a.track = t
		}
		// Volume selalu dari mpv (via PlayerService cache yg di-sync tiap 500ms)
		a.volume = a.playerSvc.Volume()
		// Read latest cava bars (non-blocking)
		if barsCh := a.playerSvc.Bars(); barsCh != nil {
			select {
			case bars := <-barsCh:
				a.eqBars = bars
			default:
			}
		} else {
			a.eqBars = nil
		}
		a.tick++
		return a, pollPlayerTick()

	case playDoneMsg:
		if msg.err != nil {
			a.err = msg.err
			return a, nil
		}
		a.cacheSvc.AddHistory(msg.vid)
		a.cacheSvc.Save()
		return a, nil
	}

	var cmd tea.Cmd
	a.search, cmd = a.search.Update(msg)
	return a, cmd
}

func (a *App) handleKey(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	// ── Quit: always ──
	if key.Matches(msg, DefaultKeyMap.Quit) {
		return a, tea.Quit
	}

	// ── Search focused: typing mode ──
	// Only F-keys & arrows for control; EVERYTHING ELSE types into search box
	if a.search.Focused() {
		switch {
		case msg.Type == tea.KeyF1:
			a.toggleMute()
			return a, nil
		case msg.Type == tea.KeyF2:
			a.volume = max(a.volume-5, 0)
			a.playerSvc.SetVolume(a.volume)
			return a, nil
		case msg.Type == tea.KeyF3:
			a.volume = min(a.volume+5, 100)
			a.playerSvc.SetVolume(a.volume)
			return a, nil
		case msg.Type == tea.KeyF4:
			a.playerSvc.Stop()
			a.track = nil
			a.state = stateSearch
			return a, nil
		case msg.Type == tea.KeyLeft && a.state == statePlayer:
			a.playerSvc.Seek(-5)
			return a, nil
		case msg.Type == tea.KeyRight && a.state == statePlayer:
			a.playerSvc.Seek(5)
			return a, nil
		case msg.Type == tea.KeyUp:
			if a.cursor > 0 {
				a.cursor--
			}
			return a, nil
		case msg.Type == tea.KeyDown:
			if a.cursor < len(a.results)-1 {
				a.cursor++
			}
			return a, nil
		case key.Matches(msg, DefaultKeyMap.Select):
			if a.fetching {
				return a, nil
			}
			q := a.search.Value()
			if q != "" {
				a.fetching = true
				a.err = nil
				return a, func() tea.Msg {
					results, err := a.searchSvc.Search(q, searchResultLimit)
					return searchDoneMsg{results: results, err: err}
				}
			}
			return a, nil
		case key.Matches(msg, DefaultKeyMap.FocusSearch):
			return a, nil
		}
		// Semua character key masuk sini — termasuk spasi, j, k, p, s, v, +, =
		var cmd tea.Cmd
		a.search, cmd = a.search.Update(msg)
		return a, cmd
	}

	// ── Search NOT focused: semua key jadi action ──
	switch {
	case key.Matches(msg, DefaultKeyMap.FocusSearch):
		a.search.Focus()
		return a, nil

	case key.Matches(msg, DefaultKeyMap.CursorUp):
		if a.cursor > 0 {
			a.cursor--
		}
		return a, nil
	case key.Matches(msg, DefaultKeyMap.CursorDown):
		if a.cursor < len(a.results)-1 {
			a.cursor++
		}
		return a, nil

	case key.Matches(msg, DefaultKeyMap.Select):
		return a.playSelected()

	case key.Matches(msg, DefaultKeyMap.PlayPause):
		if a.state == statePlayer && a.track != nil {
			a.playerSvc.Pause()
		} else if len(a.results) > 0 {
			return a.playSelected()
		}
		return a, nil

	case key.Matches(msg, DefaultKeyMap.Stop):
		a.playerSvc.Stop()
		a.track = nil
		a.state = stateSearch
		return a, nil

	case key.Matches(msg, DefaultKeyMap.SeekBack):
		if a.state == statePlayer {
			a.playerSvc.Seek(-5)
		}
		return a, nil
	case key.Matches(msg, DefaultKeyMap.SeekForward):
		if a.state == statePlayer {
			a.playerSvc.Seek(5)
		}
		return a, nil

	case key.Matches(msg, DefaultKeyMap.VolumeUp):
		a.volume = min(a.volume+5, 100)
		a.playerSvc.SetVolume(a.volume)
		return a, nil
	case key.Matches(msg, DefaultKeyMap.VolumeDown):
		a.volume = max(a.volume-5, 0)
		a.playerSvc.SetVolume(a.volume)
		return a, nil
	case key.Matches(msg, DefaultKeyMap.Mute):
		a.toggleMute()
		return a, nil
	}

	return a, nil
}

// toggleMute switches between muted and previous volume.
func (a *App) toggleMute() {
	if a.playerSvc.Volume() > 0 {
		a.mutedVolume = a.volume
		a.playerSvc.SetVolume(0)
	} else {
		vol := a.mutedVolume
		if vol == 0 {
			vol = 30
		}
		a.playerSvc.SetVolume(vol)
	}
}

func (a *App) playSelected() (tea.Model, tea.Cmd) {
	if a.cursor < 0 || a.cursor >= len(a.results) {
		return a, nil
	}
	vid := a.results[a.cursor]

	a.state = statePlayer
	return a, tea.Batch(
		func() tea.Msg {
			if err := a.playerSvc.Play(vid); err != nil {
				return playDoneMsg{vid: vid, err: err}
			}
			return playDoneMsg{vid: vid}
		},
		pollPlayerTick(),
	)
}

func pollPlayerTick() tea.Cmd {
	return tea.Tick(100*time.Millisecond, func(t time.Time) tea.Msg {
		return playerTickMsg{}
	})
}
