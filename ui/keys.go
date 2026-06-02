package ui

import "github.com/charmbracelet/bubbles/key"

// KeyMap defines all keyboard shortcuts for the app.
// Alphabet keys (A-Z) are NEVER used — reserved for search typing.
type KeyMap struct {
	Quit        key.Binding
	FocusSearch key.Binding
	CursorUp    key.Binding
	CursorDown  key.Binding
	Select      key.Binding
	PlayPause   key.Binding
	Stop        key.Binding
	SeekBack    key.Binding
	SeekForward key.Binding
	VolumeUp    key.Binding
	VolumeDown  key.Binding
	Mute        key.Binding
}

// DefaultKeyMap is the canonical set of keybindings.
// Only F-keys, symbols, arrows, Enter, Space, and Ctrl — NO letters A-Z.
var DefaultKeyMap = KeyMap{
	Quit: key.NewBinding(
		key.WithKeys("ctrl+c"),
		key.WithHelp("Ctrl+C", "quit"),
	),
	FocusSearch: key.NewBinding(
		key.WithKeys("/"),
		key.WithHelp("/", "search"),
	),
	CursorUp: key.NewBinding(
		key.WithKeys("up"),
		key.WithHelp("↑", "up"),
	),
	CursorDown: key.NewBinding(
		key.WithKeys("down"),
		key.WithHelp("↓", "down"),
	),
	Select: key.NewBinding(
		key.WithKeys("enter"),
		key.WithHelp("Enter", "play audio"),
	),
	PlayPause: key.NewBinding(
		key.WithKeys(" "),
		key.WithHelp("Space", "play/pause"),
	),
	Stop: key.NewBinding(
		key.WithKeys("f4"),
		key.WithHelp("F4", "stop"),
	),
	SeekBack: key.NewBinding(
		key.WithKeys("left"),
		key.WithHelp("←", "rewind 5s"),
	),
	SeekForward: key.NewBinding(
		key.WithKeys("right"),
		key.WithHelp("→", "forward 5s"),
	),
	VolumeUp: key.NewBinding(
		key.WithKeys("+", "=", "f3"),
		key.WithHelp("F3/+", "vol up"),
	),
	VolumeDown: key.NewBinding(
		key.WithKeys("-", "f2"),
		key.WithHelp("F2/-", "vol down"),
	),
	Mute: key.NewBinding(
		key.WithKeys("f1"),
		key.WithHelp("F1", "mute"),
	),
}
