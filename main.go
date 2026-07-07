package main

import (
	"embed"

	"log"
	"time"

	"github.com/wailsapp/wails/v3/pkg/application"
)

// Wails uses Go's `embed` package to embed the frontend files into the binary.
// Any files in the frontend/dist folder will be embedded into the binary and
// made available to the frontend.
// See https://pkg.go.dev/embed for more information.

//go:embed all:frontend/dist
var assets embed.FS

func init() {
	application.RegisterEvent[string]("clipboard-history-changed")
}

func macApplicationOptions() application.MacOptions {
	return application.MacOptions{
		ActivationPolicy: application.ActivationPolicyRegular,
		ApplicationShouldTerminateAfterLastWindowClosed: false,
	}
}

func mainWindowOptions() application.WebviewWindowOptions {
	const (
		width  = 1000
		height = 618
	)

	return application.WebviewWindowOptions{
		Title: "ntools",
		// Window sized to the golden ratio (1000 / 618 ≈ 1.618).
		Width:         width,
		Height:        height,
		MinWidth:      width,
		MinHeight:     height,
		MaxWidth:      width,
		MaxHeight:     height,
		DisableResize: true,
		Mac: application.MacWindow{
			InvisibleTitleBarHeight: 50,
			Backdrop:                application.MacBackdropNormal,
			TitleBar:                application.MacTitleBarHiddenInset,
		},
		BackgroundColour: application.NewRGBA(6, 7, 15, 255),
		URL:              "/",
	}
}

// main function serves as the application's entry point. It initializes the application, creates a window,
// and starts a goroutine that emits a time-based event every second. It subsequently runs the application and
// logs any error that might occur.
func main() {
	if runCopyDebugMode() {
		return
	}
	if err := ensureMacAppBundleLaunch(); err != nil {
		log.Printf("mac app bundle relaunch failed: %v", err)
	}

	historyPath, err := clipboardHistoryPath()
	if err != nil {
		debugPrintf("[ntools] clipboard history disabled: %v\n", err)
		log.Printf("clipboard history disabled: %v", err)
	} else {
		debugPrintf("[ntools] clipboard history path: %s\n", historyPath)
	}

	configPath, err := appConfigPath()
	if err != nil {
		debugPrintf("[ntools] config disabled: %v\n", err)
		log.Printf("config disabled: %v", err)
	}
	configStore := newConfigStore(configPath)
	config, err := configStore.Load()
	if err != nil {
		debugPrintf("[ntools] config load failed, using defaults: %v\n", err)
		log.Printf("config load failed, using defaults: %v", err)
		config = defaultAppConfig()
	}

	history := newHistoryService(historyPath, historyServiceOptions{MaxItems: config.History.MaxItems})
	clipboardService := NewClipboardService(history)
	configService := NewConfigService(configStore)

	// Create a new Wails application by providing the necessary options.
	// Variables 'Name' and 'Description' are for application metadata.
	// 'Assets' configures the asset server with the 'FS' variable pointing to the frontend files.
	// 'Bind' is a list of Go struct instances. The frontend has access to the methods of these instances.
	// 'Mac' options tailor the application when running an macOS.
	app := application.New(application.Options{
		Name:        "ntools",
		Description: "A demo of using raw HTML & CSS",
		Icon:        appIcon,
		Services: []application.Service{
			application.NewService(&GreetService{}),
			application.NewService(clipboardService),
			application.NewService(configService),
		},
		Assets: application.AssetOptions{
			Handler: application.AssetFileServerFS(assets),
		},
		Mac: macApplicationOptions(),
	})
	clipboardService.setClipboardWriter(app.Clipboard.SetText)

	copyMonitor := newCopyMonitor(copyMonitorOptions{
		ReadClipboard: readSystemClipboardText,
		Logf:          log.Printf,
		WriteHistory: func(record clipboardRecord) error {
			if historyPath == "" {
				return nil
			}
			if _, err := history.AddText(record.Text); err != nil {
				return err
			}
			debugPrintf("[ntools] clipboard history written: %s\n", historyPath)
			return nil
		},
		OnHistoryChange: func() {
			app.Event.Emit("clipboard-history-changed", "")
		},
		Delay:    80 * time.Millisecond,
		Debounce: 150 * time.Millisecond,
	})
	if err := startCopyShortcutListener(copyMonitor.HandleCopyShortcut); err != nil {
		debugPrintf("[ntools] copy shortcut listener disabled: %v\n", err)
		log.Printf("copy shortcut listener disabled: %v", err)
		debugPrintf("[ntools] clipboard polling fallback enabled\n")
		startClipboardPolling(readSystemClipboardText, copyMonitor.HandleCopiedText, 500*time.Millisecond)
	} else {
		debugPrintf("[ntools] copy shortcut listener enabled\n")
	}

	// Create a new window with the necessary options.
	// 'Title' is the title of the window.
	// 'Mac' options tailor the window when running on macOS.
	// 'BackgroundColour' is the background colour of the window.
	// 'URL' is the URL that will be loaded into the webview.
	mainWindow := app.Window.NewWithOptions(mainWindowOptions())
	setupSystemTray(app, mainWindow, trayIcon)

	// Run the application. This blocks until the application has been exited.
	err = app.Run()

	// If an error occurred while running the application, log it and exit.
	if err != nil {
		log.Fatal(err)
	}
}
