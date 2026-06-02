package ui

import "github.com/charmbracelet/lipgloss"

// Colour palette — matching the teal/green terminal aesthetic
const (
	colorTeal      = lipgloss.Color("#00B894")
	colorDim       = lipgloss.Color("#4A5568")
	colorText      = lipgloss.Color("#E2E8F0")
	colorMuted     = lipgloss.Color("#718096")
	colorHighlight = lipgloss.Color("#00CEC9")
	colorLive      = lipgloss.Color("#FF6B6B")
	colorBg        = lipgloss.Color("#0D1117")
)

var (
	// Outer border for all panels — NormalBorder matches tview default ┌┐└┘
	panelStyle = lipgloss.NewStyle().
			Border(lipgloss.NormalBorder()).
			BorderForeground(colorTeal)

	// Logo / title inside sidebar
	logoStyle = lipgloss.NewStyle().
			Foreground(colorTeal).
			Bold(true).
			Align(lipgloss.Center)

	// Section label inside sidebar (e.g. "Playback Control")
	sectionLabelStyle = lipgloss.NewStyle().
				Foreground(colorMuted).
				MarginTop(1)

	// Keybind line inside sidebar
	keybindStyle = lipgloss.NewStyle().
			Foreground(colorText)

	// Main content panel header
	headerStyle = lipgloss.NewStyle().
			Foreground(colorTeal).
			Bold(true).
			PaddingLeft(1)

	// Table header row
	tableHeaderStyle = lipgloss.NewStyle().
				Foreground(colorMuted).
				Bold(true).
				PaddingLeft(1)

	// Normal result row
	rowStyle = lipgloss.NewStyle().
			Foreground(colorText).
			PaddingLeft(1)

	// Selected / highlighted row
	selectedRowStyle = lipgloss.NewStyle().
				Foreground(colorBg).
				Background(colorTeal).
				Bold(true).
				PaddingLeft(1)

	// LIVE badge
	liveBadgeStyle = lipgloss.NewStyle().
			Foreground(colorLive).
			Bold(true)

	// Status bar at the bottom
	statusBarStyle = lipgloss.NewStyle().
			Foreground(colorTeal).
			Bold(true).
			PaddingLeft(1)

	// Volume bar filled portion
	volFilledStyle = lipgloss.NewStyle().Foreground(colorTeal)
	volEmptyStyle  = lipgloss.NewStyle().Foreground(colorDim)
	dimStyle       = lipgloss.NewStyle().Foreground(colorMuted)

	// Error text
	errorStyle = lipgloss.NewStyle().
			Foreground(colorLive).
			Bold(true)
)

// volumeBar renders a simple [####----] bar for the given 0-100 value.
func volumeBar(vol, width int) string {
	filled := vol * width / 100
	bar := ""
	for i := 0; i < width; i++ {
		if i < filled {
			bar += volFilledStyle.Render("#")
		} else {
			bar += volEmptyStyle.Render("-")
		}
	}
	return "[" + bar + "]"
}
