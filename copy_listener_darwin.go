//go:build darwin

package main

import "changeme/internal/clipboard"

func startCopyShortcutListener(onCopy func()) error {
	clipboard.SetDebugLogger(debugPrintf)
	return clipboard.StartCopyShortcutListener(onCopy)
}
