package clipboard

import "time"

func StartPolling(readClipboard func() (string, bool), onText func(string), interval time.Duration) {
	if readClipboard == nil || onText == nil {
		return
	}
	if interval <= 0 {
		interval = 500 * time.Millisecond
	}

	go func() {
		ticker := time.NewTicker(interval)
		defer ticker.Stop()

		var lastText string
		if text, ok := readClipboard(); ok {
			lastText = text
		}

		for range ticker.C {
			text, ok := readClipboard()
			if !ok || text == "" || text == lastText {
				continue
			}

			lastText = text
			onText(text)
		}
	}()
}
