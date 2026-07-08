package main

import (
	"time"

	"changeme/internal/clipboard"
)

func startClipboardPolling(readClipboard func() (string, bool), onText func(string), interval time.Duration) {
	if readClipboard == nil || onText == nil {
		return
	}

	clipboard.StartPolling(readClipboard, func(text string) {
		debugPrintf("[cv-fun] clipboard changed via polling\n")
		onText(text)
	}, interval)
}
