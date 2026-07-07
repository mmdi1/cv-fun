//go:build darwin

package main

import "changeme/internal/clipboard"

func readSystemClipboardText() (string, bool) {
	return clipboard.ReadText()
}
