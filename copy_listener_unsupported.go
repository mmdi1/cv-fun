//go:build !darwin && !windows

package main

import "changeme/internal/clipboard"

func startCopyShortcutListener(onCopy func()) error {
	clipboard.SetDebugLogger(debugPrintf)
	return clipboard.StartCopyShortcutListener(onCopy)
}
