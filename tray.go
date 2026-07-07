package main

import (
	_ "embed"
	"runtime"
	"sync/atomic"

	"github.com/wailsapp/wails/v3/pkg/application"
	"github.com/wailsapp/wails/v3/pkg/events"
)

//go:embed build/appicon.png
var appIcon []byte

//go:embed build/trayicon.png
var trayIcon []byte

type trayLifecycle struct {
	quitting atomic.Bool
}

func newTrayLifecycle() *trayLifecycle {
	return &trayLifecycle{}
}

func (l *trayLifecycle) handleWindowClose(hidePanel func()) bool {
	if l.quitting.Load() {
		return false
	}
	if hidePanel != nil {
		hidePanel()
	}
	return true
}

func (l *trayLifecycle) requestQuit(quitApp func()) {
	l.quitting.Store(true)
	if quitApp != nil {
		quitApp()
	}
}

func (l *trayLifecycle) showFromTray(restoreAppIcon func(), showWindow func(), showDockIcon func()) {
	if restoreAppIcon != nil {
		restoreAppIcon()
	}
	if showWindow != nil {
		showWindow()
	}
	if showDockIcon != nil {
		showDockIcon()
	}
}

func (l *trayLifecycle) handleTrayClick(isWindowVisible func() bool, restoreAppIcon func(), showWindow func(), showDockIcon func(), hidePanel func()) {
	if isWindowVisible != nil && isWindowVisible() {
		if hidePanel != nil {
			hidePanel()
		}
		return
	}
	l.showFromTray(restoreAppIcon, showWindow, showDockIcon)
}

func trayMenuLabels() []string {
	return []string{"显示 ntools", "退出"}
}

func newTrayMenu(lifecycle *trayLifecycle, showWindow func(), quitApp func()) *application.Menu {
	labels := trayMenuLabels()
	menu := application.NewMenu()
	menu.Add(labels[0]).OnClick(func(ctx *application.Context) {
		if showWindow != nil {
			showWindow()
		}
	})
	menu.AddSeparator()
	menu.Add(labels[1]).OnClick(func(ctx *application.Context) {
		lifecycle.requestQuit(quitApp)
	})
	return menu
}

func setupSystemTray(app *application.App, window application.Window, icon []byte) *application.SystemTray {
	lifecycle := newTrayLifecycle()
	applyRuntimeAppIcon := func() {
		applyAppIcon(appIcon)
		app.SetIcon(appIcon)
	}
	applyRuntimeWindowIcon := func() {
		applyWindowAnimationIcon(window, appIcon)
	}

	window.RegisterHook(events.Common.WindowClosing, func(event *application.WindowEvent) {
		if lifecycle.handleWindowClose(
			func() {
				hidePanelWithoutDockBounce(window)
			},
		) {
			event.Cancel()
		}
	})
	window.RegisterHook(events.Mac.WebViewDidFinishNavigation, func(event *application.WindowEvent) {
		applyRuntimeAppIcon()
		applyRuntimeWindowIcon()
		disableWindowAnimations(window)
	})

	tray := app.SystemTray.New()
	tray.SetTooltip("ntools")
	if len(icon) > 0 {
		if runtime.GOOS == "darwin" {
			tray.SetTemplateIcon(icon)
		} else {
			tray.SetIcon(icon)
		}
	}
	tray.OnClick(func() {
		lifecycle.handleTrayClick(
			window.IsVisible,
			applyRuntimeAppIcon,
			func() {
				showWindowWithoutAnimation(window)
			},
			showDockIcon,
			func() {
				hidePanelWithoutDockBounce(window)
			},
		)
	})
	tray.SetMenu(newTrayMenu(lifecycle, func() {
		lifecycle.showFromTray(
			applyRuntimeAppIcon,
			func() {
				showWindowWithoutAnimation(window)
			},
			showDockIcon,
		)
	}, app.Quit))

	return tray
}
