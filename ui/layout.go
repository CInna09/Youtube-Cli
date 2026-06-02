package ui

import (
	"fmt"
	"math"
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// View renders the full screen.
//
//	┌──────────────────────────────────────┐
//	│  sidebar (20) │  main                │
//	│               │                      │
//	├───────────────┴──────────────────────┤
//	│  status bar                          │
//	└──────────────────────────────────────┘
func (a *App) View() string {
	if a.width == 0 {
		return "loading..."
	}

	// ── Dimensions ──
	// Outer panel inner width = terminal - 2 borders
	outerInnerW := a.width - 2
	if outerInnerW < 10 {
		outerInnerW = 10
	}
	// Sidebar content width + 1 for separator column = 21
	sidebarContentW := 20
	// Main gets the rest (minus 1 for the separator column)
	mainContentW := outerInnerW - sidebarContentW - 1

	// Height: top panel = all but 3 (status), inner = total - 2 borders
	wholeInnerH := a.height - 3 - 2 // -status(3) -top/bot borders(2)
	if wholeInnerH < 1 {
		wholeInnerH = 1
	}

	// ── Sidebar content ──
	sideText := lipgloss.NewStyle().
		Width(sidebarContentW).
		Render(a.renderSidebarContent())

	// Separator line between sidebar and main
	sep := lipgloss.NewStyle().
		Foreground(colorTeal).
		Width(1).
		Align(lipgloss.Center).
		Render("│")

	// ── Main content with dynamic column widths ──
	mainStr := a.renderMainContent(mainContentW)
	mainText := lipgloss.NewStyle().
		Width(mainContentW).
		Render(mainStr)

	// ── Join sidebar + separator + main ──
	combined := lipgloss.JoinHorizontal(lipgloss.Top, sideText, sep, mainText)

	// ── Top panel: full outer border ──
	topPanel := panelStyle.
		Width(outerInnerW).
		Height(wholeInnerH).
		MaxHeight(wholeInnerH).
		Render(combined)

	// ── Status bar: render text at full width so no gap before right border ──
	statusRaw := a.renderStatusContent(outerInnerW)
	statusPanel := panelStyle.
		Width(outerInnerW).
		Height(1).
		Render(statusRaw)

	return lipgloss.JoinVertical(lipgloss.Left, topPanel, statusPanel)
}

// ── Sidebar inner content ──

func (a *App) renderSidebarContent() string {
	logo := logoStyle.Render("┌──────────────┐\n│  ( ─ . ─ )   │\n│   YTcliV2    │\n└──────────────┘")

	kb := sectionLabelStyle.Render("Playback Control") + "\n" +
		keybindStyle.Render("[F1] Mute") + "\n" +
		keybindStyle.Render("[F2] Vol-") + "\n" +
		keybindStyle.Render("[F3/+] Vol+") + "\n" +
		keybindStyle.Render("[←] Rewind 5s") + "\n" +
		keybindStyle.Render("[→] Forward 5s") + "\n" +
		keybindStyle.Render("[Space] P/Pause") + "\n" +
		keybindStyle.Render("[F4] Stop") + "\n" +
		keybindStyle.Render("[Enter] Play") + "\n" +
		keybindStyle.Render("[/] Search") + "\n" +
		keybindStyle.Render("[Ctrl+C] Quit")

	content := lipgloss.JoinVertical(lipgloss.Left,
		logo,
		kb,
	)

	return content
}

// ── Main panel inner content (width = available chars for this area) ──

func (a *App) renderMainContent(w int) string {
	query := a.search.Value()
	if query == "" {
		query = "Player"
	}
	header := headerStyle.Render(fmt.Sprintf("Search Results '%s'", query))

	// Search bar — input rendered directly, no extra border
	searchBar := a.search.View()

	// Loading
	if a.fetching {
		return lipgloss.JoinVertical(lipgloss.Left,
			header,
			searchBar,
			"\n  Searching...",
		)
	}

	// Error
	if a.err != nil {
		errText := errorStyle.Render("Error: " + a.err.Error())
		return lipgloss.JoinVertical(lipgloss.Left,
			header,
			searchBar,
			"\n"+errText,
		)
	}

	// Idle — push content down with padding to avoid looking empty
	if len(a.results) == 0 {
		return lipgloss.JoinVertical(lipgloss.Left,
			header,
			searchBar,
			"\n\n\n  Type a query and press Enter to search",
		)
	}

	// ── Dynamic column widths based on available space ──
	// Fixed: Index(5) + Duration(8) + 4 inter-column spaces = 17
	// Remaining split between Title, Channel, Views
	idxW := 5
	durW := 8
	fixed := idxW + durW + 4 // +4 for spaces between cols
	rem := w - fixed
	if rem < 20 {
		rem = 20
	}
	titleW := rem * 55 / 100 // 55%
	chW := rem * 25 / 100    // 25%

	fmtHead := fmt.Sprintf("%%-%ds %%-%ds %%-%ds %%-%ds %%s", idxW, titleW, chW, durW)
	fmtRow := fmt.Sprintf("%%-%ds %%-%ds %%-%ds %%-%ds %%s", idxW, titleW, chW, durW)

	// Table header
	tableHeader := tableHeaderStyle.Render(
		fmt.Sprintf(fmtHead, "Index", "Title", "Channel", "Duration", "Views"),
	)

	// Rows
	rows := ""
	for i, v := range a.results {
		dur := v.Duration
		if v.IsLive {
			dur = liveBadgeStyle.Render("LIVE")
		}
		title := truncate(v.Title, titleW)
		ch := truncate(v.Channel, chW)
		line := fmt.Sprintf(fmtRow,
			fmt.Sprintf("[%d]", i+1),
			title,
			fmt.Sprintf("[%s]", ch),
			dur,
			fmt.Sprintf("[%s]", v.Views),
		)
		if i == a.cursor {
			rows += selectedRowStyle.Render(line) + "\n"
		} else {
			rows += rowStyle.Render(line) + "\n"
		}
	}

	content := lipgloss.JoinVertical(lipgloss.Left,
		header,
		searchBar,
		tableHeader,
		strings.TrimRight(rows, "\n"),
	)

	return content
}

// ── Status bar inner content (width = full available space) ──
// Layout: [eq bars left]  Title | time | Vol  [eq bars right]
// Eq bars repeat to fill all empty space so no gap before right border.

func (a *App) renderStatusContent(width int) string {
	if a.track == nil {
		return statusBarStyle.
			Width(width).
			Render("No track playing  •  Press / to search")
	}

	v := a.track
	pos := formatDuration(v.Position)
	dur := formatDuration(v.Duration)

	pauseTag := ""
	if v.Paused {
		pauseTag = "  [PAUSED]"
	}

	// Info text without equalizer
	info := fmt.Sprintf("  %s  |  %s / %s%s  |  Vol: %d%%  ",
		v.Video.Title, pos, dur, pauseTag, a.volume)
	infoW := lipgloss.Width(statusBarStyle.Render(info))

	// Remaining width for eq bars
	eqTotal := width - infoW
	if eqTotal < 2 {
		eqTotal = 2
	}
	leftW := eqTotal / 2
	rightW := eqTotal - leftW

	leftEq := renderEqBars(leftW, a.tick, 0, v.Paused, a.eqBars)
	rightEq := renderEqBars(rightW, a.tick, float64(leftW)*0.2, v.Paused, a.eqBars)

	full := leftEq + info + rightEq
	return statusBarStyle.Width(width).Render(full)
}

// renderEqBars generates N single-char eq bars (no spaces between).
// When bars (cava data, values 0-1000) is non-nil, it uses those real values instead of
// synthetic sine waves. bars is shared across calls so callers pass the same slice.
// phaseOffset shifts the animation for variety between left/right halves.
func renderEqBars(count int, tick uint64, phaseOffset float64, paused bool, bars []int) string {
	if count < 1 {
		return ""
	}
	blocks := []string{"▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"}

	// ── Real bar data from cava ──
	if len(bars) > 0 {
		var out string
		for i := 0; i < count; i++ {
			if paused {
				out += dimStyle.Render("▁")
				continue
			}
			// Map across the available bar data
			idx := i * len(bars) / count
			if idx >= len(bars) {
				idx = len(bars) - 1
			}
			// Scale 0-1000 → 0-7
			bi := bars[idx] * 7 / 1000
			if bi < 0 {
				bi = 0
			}
			if bi > 7 {
				bi = 7
			}
			if bi >= 3 {
				out += volFilledStyle.Render(blocks[bi])
			} else {
				out += dimStyle.Render(blocks[bi])
			}
		}
		return out
	}

	// ── Synthetic sine-wave fallback ──
	t := float64(tick) * 0.08
	var out string
	for i := 0; i < count; i++ {
		if paused {
			out += dimStyle.Render("▁")
			continue
		}
		p := phaseOffset + float64(i)*0.4
		v := math.Sin(t + p)
		idx := int((v + 1) * 3.5)
		if idx < 0 {
			idx = 0
		}
		if idx > 7 {
			idx = 7
		}
		if idx >= 3 {
			out += volFilledStyle.Render(blocks[idx])
		} else {
			out += dimStyle.Render(blocks[idx])
		}
	}
	return out
}

// ── Helpers ──

func truncate(s string, max int) string {
	if len(s) <= max {
		return s
	}
	return s[:max-1] + "…"
}

func formatDuration(secs float64) string {
	t := int(secs)
	h := t / 3600
	m := (t % 3600) / 60
	s := t % 60
	if h > 0 {
		return fmt.Sprintf("%d:%02d:%02d", h, m, s)
	}
	return fmt.Sprintf("%02d:%02d", m, s)
}
