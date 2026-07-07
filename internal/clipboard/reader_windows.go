//go:build windows

package clipboard

import "github.com/wailsapp/wails/v3/pkg/w32"

func ReadText() (string, bool) {
	text, err := w32.GetClipboardText()
	return text, err == nil && text != ""
}
