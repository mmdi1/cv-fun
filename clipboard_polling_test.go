package main

import (
	"testing"
	"time"
)

func TestClipboardPollingSkipsInitialClipboardText(t *testing.T) {
	values := []string{"old crash log", "old crash log", "new copy"}
	readIndex := 0
	seen := make(chan string, 1)

	startClipboardPolling(
		func() (string, bool) {
			if readIndex >= len(values) {
				return values[len(values)-1], true
			}
			value := values[readIndex]
			readIndex++
			return value, true
		},
		func(text string) {
			seen <- text
		},
		5*time.Millisecond,
	)

	select {
	case text := <-seen:
		if text != "new copy" {
			t.Fatalf("expected only changed clipboard text, got %q", text)
		}
	case <-time.After(100 * time.Millisecond):
		t.Fatal("expected changed clipboard text to be observed")
	}
}
