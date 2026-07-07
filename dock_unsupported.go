//go:build !darwin

package main

import "github.com/wailsapp/wails/v3/pkg/application"

func hideDockIcon() {}

func showDockIcon() {}

func applyAppIcon(icon []byte) {}

func applyWindowAnimationIcon(window application.Window, icon []byte) {}

func disableWindowAnimations(window application.Window) {}

func showWindowWithoutAnimation(window application.Window) {
	if window != nil {
		window.Show().Focus()
	}
}

func hideWindowWithoutAnimation(window application.Window) {
	if window != nil {
		window.Hide()
	}
}

func hidePanelWithoutDockBounce(window application.Window) {
	hideDockIcon()
	hideWindowWithoutAnimation(window)
}
