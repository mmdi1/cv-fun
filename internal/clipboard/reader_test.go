package clipboard

import "testing"

func TestReadTextReturnsTextAndAvailabilityFlag(t *testing.T) {
	text, ok := ReadText()
	if !ok && text != "" {
		t.Fatalf("expected empty text when clipboard text is unavailable, got %q", text)
	}
}
