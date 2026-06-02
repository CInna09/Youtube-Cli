package main

import (
	"fmt"
	"os"

	tea "github.com/charmbracelet/bubbletea"

	"YTcliV2/infra"
	"YTcliV2/service"
	"YTcliV2/ui"
)

func main() {
	// ── Config ──
	cfg, err := infra.Load()
	if err != nil {
		fmt.Fprintf(os.Stderr, "config: %v\n", err)
		os.Exit(1)
	}

	// ── Infra ──
	ytdlp := infra.NewYtdlpClient()
	ytdlp.ExtraArgs = cfg.ExtraArgs
	mpv := infra.NewMpvClient(cfg.SocketPath, "mpv")

	// Try starting mpv (non-fatal if missing)
	if err := mpv.Start(); err != nil {
		fmt.Fprintf(os.Stderr, "mpv not available (%v)\n", err)
		fmt.Fprintf(os.Stderr, "  Install mpv and try again.\n")
	}

	// ── Audio visualiser (cava) ──
	cava := infra.NewCavaClient()

	// ── Services ──
	searchSvc := service.NewSearchService(ytdlp)
	playerSvc := service.NewPlayerService(mpv, searchSvc, cava, cfg)

	homeDir, _ := os.UserHomeDir()
	cacheDir := homeDir + "/.config/YTcliV2"
	cacheSvc := service.NewCacheService(cacheDir)

	if err := cacheSvc.Load(); err != nil {
		fmt.Fprintf(os.Stderr, "cache: %v\n", err)
	}

	// ── TUI ──
	p := tea.NewProgram(
		ui.New(searchSvc, playerSvc, cacheSvc),
		tea.WithAltScreen(),
		tea.WithMouseCellMotion(),
	)

	if _, err := p.Run(); err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}

	// ── Cleanup ──
	mpv.Quit()
}
