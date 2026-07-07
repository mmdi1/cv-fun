//go:build !darwin && !windows

package main

import "changeme/internal/clipboard"

func readSystemClipboardText() (string, bool) {
	return clipboard.ReadText()
}
