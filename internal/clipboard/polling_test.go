package clipboard

import (
	"testing"
	"time"
)

func TestStartPollingSkipsInitialClipboardText(t *testing.T) {
	values := []string{"old text", "old text", "new copy"}
	readIndex := 0
	seen := make(chan string, 1)

	StartPolling(
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
