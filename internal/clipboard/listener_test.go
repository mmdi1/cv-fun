package clipboard

import "testing"

func TestStartCopyShortcutListenerRejectsNilCallback(t *testing.T) {
	if err := StartCopyShortcutListener(nil); err == nil {
		t.Fatal("expected nil copy shortcut callback to be rejected")
	}
}
