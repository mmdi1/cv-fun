//go:build !darwin && !windows

package clipboard

import "errors"

func StartCopyShortcutListener(onCopy func()) error {
	if onCopy == nil {
		return errors.New("copy shortcut listener requires a callback")
	}

	return errors.New("copy shortcut listener is only supported on macOS and Windows")
}
