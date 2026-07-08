package main

import (
	"bytes"
	"image/color"
	"image/png"
	"os"
	"strings"
	"testing"

	"github.com/wailsapp/wails/v3/pkg/application"
)

func TestTrayLifecycleHidesWindowOnCloseUntilQuitIsRequested(t *testing.T) {
	lifecycle := newTrayLifecycle()
	calls := []string{}

	cancelClose := lifecycle.handleWindowClose(
		func() {
			calls = append(calls, "hide-panel")
		},
	)

	if !cancelClose {
		t.Fatalf("expected normal window close to be cancelled")
	}
	if len(calls) != 1 || calls[0] != "hide-panel" {
		t.Fatalf("expected close to hide the panel without redrawing Dock icons first, got %#v", calls)
	}

	lifecycle.requestQuit(func() {})
	cancelClose = lifecycle.handleWindowClose(
		func() {
			calls = append(calls, "hide-panel")
		},
	)

	if cancelClose {
		t.Fatalf("expected explicit quit to allow the close event")
	}
	if len(calls) != 1 {
		t.Fatalf("expected explicit quit not to run hide callbacks again, got %#v", calls)
	}
}

func TestTrayLifecycleShowsWindowBeforeRestoringDock(t *testing.T) {
	lifecycle := newTrayLifecycle()
	calls := []string{}

	lifecycle.showFromTray(
		func() {
			calls = append(calls, "icon")
		},
		func() {
			calls = append(calls, "window")
		},
		func() {
			calls = append(calls, "dock")
		},
	)

	if len(calls) != 3 || calls[0] != "icon" || calls[1] != "window" || calls[2] != "dock" {
		t.Fatalf("expected tray show to restore the app icon, show the window, then restore Dock, got %#v", calls)
	}
}

func TestTrayLifecycleTogglesTrayClickWithoutRepositioning(t *testing.T) {
	lifecycle := newTrayLifecycle()
	calls := []string{}

	lifecycle.handleTrayClick(
		func() bool {
			return false
		},
		func() {
			calls = append(calls, "icon")
		},
		func() {
			calls = append(calls, "show-window")
		},
		func() {
			calls = append(calls, "show-dock")
		},
		func() {
			calls = append(calls, "hide-panel")
		},
	)

	if len(calls) != 3 || calls[0] != "icon" || calls[1] != "show-window" || calls[2] != "show-dock" {
		t.Fatalf("expected hidden window tray click to show in place, got %#v", calls)
	}

	calls = []string{}
	lifecycle.handleTrayClick(
		func() bool {
			return true
		},
		func() {
			calls = append(calls, "icon")
		},
		func() {
			calls = append(calls, "show-window")
		},
		func() {
			calls = append(calls, "show-dock")
		},
		func() {
			calls = append(calls, "hide-panel")
		},
	)

	if len(calls) != 1 || calls[0] != "hide-panel" {
		t.Fatalf("expected visible window tray click to hide the panel without redrawing Dock icons first, got %#v", calls)
	}
}

func TestTrayMenuLabelsExposeShowAndQuit(t *testing.T) {
	labels := trayMenuLabels()

	if len(labels) != 2 {
		t.Fatalf("expected two menu labels, got %#v", labels)
	}
	if labels[0] != "显示 cv-fun" {
		t.Fatalf("expected first tray menu label to show cv-fun, got %q", labels[0])
	}
	if labels[1] != "退出" {
		t.Fatalf("expected second tray menu label to quit, got %q", labels[1])
	}
}

func TestTrayIconUsesTransparentBackground(t *testing.T) {
	image, err := png.Decode(bytes.NewReader(trayIcon))
	if err != nil {
		t.Fatalf("decode tray icon png: %v", err)
	}

	_, _, _, alpha := image.At(0, 0).RGBA()
	if alpha != 0 {
		t.Fatalf("expected tray icon corner to be transparent, alpha=%d", alpha)
	}
}

func TestAppIconUsesTransparentOuterBackground(t *testing.T) {
	image, err := png.Decode(bytes.NewReader(appIcon))
	if err != nil {
		t.Fatalf("decode app icon png: %v", err)
	}

	_, _, _, cornerAlpha := image.At(0, 0).RGBA()
	if cornerAlpha != 0 {
		t.Fatalf("expected app icon corner to be transparent, alpha=%d", cornerAlpha)
	}

	_, _, _, centerAlpha := image.At(image.Bounds().Dx()/2, image.Bounds().Dy()/2).RGBA()
	if centerAlpha == 0 {
		t.Fatalf("expected app icon center to stay visible")
	}

	bodyColor := color.RGBAModel.Convert(image.At(image.Bounds().Dx()/2, image.Bounds().Dy()/5)).(color.RGBA)
	if bodyColor.R > 80 || bodyColor.G > 90 || bodyColor.B > 110 {
		t.Fatalf("expected app icon body to stay dark, got %#v", bodyColor)
	}
}

func TestMacApplicationOptionsKeepDockIconVisible(t *testing.T) {
	options := macApplicationOptions()

	if options.ActivationPolicy != application.ActivationPolicyRegular {
		t.Fatalf("expected mac app to keep the Dock icon visible, got policy %d", options.ActivationPolicy)
	}
	if options.ApplicationShouldTerminateAfterLastWindowClosed {
		t.Fatalf("expected mac app to keep running after the panel is closed")
	}
}

func TestMainWindowOptionsUseStableOpaqueMacBackdrop(t *testing.T) {
	options := mainWindowOptions()

	if options.Mac.Backdrop != application.MacBackdropNormal {
		t.Fatalf("expected main window to avoid translucent macOS backdrop, got %d", options.Mac.Backdrop)
	}
	if options.BackgroundColour.Red != 6 || options.BackgroundColour.Green != 7 || options.BackgroundColour.Blue != 15 || options.BackgroundColour.Alpha != 255 {
		t.Fatalf("expected stable opaque background colour, got %#v", options.BackgroundColour)
	}
	if !options.DisableResize {
		t.Fatalf("expected fixed-size panel to avoid native border resize jitter")
	}
	if options.MinWidth != options.Width || options.MaxWidth != options.Width || options.MinHeight != options.Height || options.MaxHeight != options.Height {
		t.Fatalf("expected min/max size to match initial size, got %#v", options)
	}
}

func TestDarwinPanelHideCombinesDockPolicyAndWindowOrdering(t *testing.T) {
	source, err := os.ReadFile("dock_darwin.go")
	if err != nil {
		t.Fatalf("read darwin dock source: %v", err)
	}
	code := string(source)

	if !strings.Contains(code, "cvfunHidePanelWithoutDockBounce") {
		t.Fatalf("expected a native helper that hides the Dock icon and window together")
	}
	helperStart := strings.Index(code, "static void cvfunHidePanelWithoutDockBounce")
	if helperStart < 0 {
		t.Fatalf("expected native hide-panel helper definition")
	}
	helperEnd := strings.Index(code[helperStart:], "static void cvfun")
	if helperEnd <= 0 {
		helperEnd = len(code) - helperStart
	}
	helper := code[helperStart : helperStart+helperEnd]

	if !strings.Contains(helper, "NSApplicationActivationPolicyAccessory") || !strings.Contains(helper, "orderOut:nil") {
		t.Fatalf("expected native hide-panel helper to switch the app to Accessory and order the window out in one main-thread block")
	}
}

func TestClipboardPanelUsesStableViewportLayout(t *testing.T) {
	source, err := os.ReadFile("frontend/src/features/clipboard/ClipboardPanel.vue")
	if err != nil {
		t.Fatalf("read clipboard panel source: %v", err)
	}
	css := string(source)

	if strings.Contains(css, "100dvh") {
		t.Fatalf("expected clipboard panel to avoid dynamic viewport units that can resize after show")
	}
	if !strings.Contains(css, "height: 100vh;") {
		t.Fatalf("expected clipboard panel root to use stable viewport height")
	}
	if !strings.Contains(css, "height: 100%;") {
		t.Fatalf("expected workspace to fill the stable root height")
	}
}

func TestClipboardPanelDefinesAppFontReset(t *testing.T) {
	source, err := os.ReadFile("frontend/src/features/clipboard/ClipboardPanel.vue")
	if err != nil {
		t.Fatalf("read clipboard panel source: %v", err)
	}
	css := string(source)

	if !strings.Contains(css, "-apple-system") || !strings.Contains(css, "BlinkMacSystemFont") {
		t.Fatalf("expected clipboard panel to define the app system font stack")
	}
	if !strings.Contains(css, "font: inherit;") {
		t.Fatalf("expected controls to inherit the app font instead of browser defaults")
	}
	if !strings.Contains(css, "-webkit-font-smoothing: antialiased") {
		t.Fatalf("expected app font smoothing to be restored after removing template CSS")
	}
}

func TestFrontendEntryDoesNotLoadTemplateGlobalCSS(t *testing.T) {
	source, err := os.ReadFile("frontend/index.html")
	if err != nil {
		t.Fatalf("read frontend index: %v", err)
	}
	html := string(source)

	if strings.Contains(html, "/style.css") {
		t.Fatalf("expected frontend entry not to load Wails template global CSS")
	}
	if strings.Contains(html, `class="bg"`) {
		t.Fatalf("expected frontend entry not to render the unused template background layer")
	}
}
