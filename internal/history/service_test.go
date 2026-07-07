package history

import (
	"path/filepath"
	"testing"
	"time"
)

func TestServiceAddsAndListsNewestFirst(t *testing.T) {
	now := time.Unix(200, 0).UTC()
	service := NewService(filepath.Join(t.TempDir(), "history.json"), ServiceOptions{
		Now: func() time.Time {
			return now
		},
	})

	first, err := service.AddText("first")
	if err != nil {
		t.Fatalf("add first: %v", err)
	}

	now = time.Unix(201, 0).UTC()
	second, err := service.AddText("second")
	if err != nil {
		t.Fatalf("add second: %v", err)
	}

	items, err := service.List("")
	if err != nil {
		t.Fatalf("list history: %v", err)
	}

	if len(items) != 2 {
		t.Fatalf("expected 2 items, got %d: %#v", len(items), items)
	}
	if items[0].ID != second.ID || items[1].ID != first.ID {
		t.Fatalf("expected newest first, got %#v", items)
	}
}
