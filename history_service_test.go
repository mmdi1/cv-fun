package main

import (
	"path/filepath"
	"testing"
	"time"
)

func TestHistoryServiceAddsAndListsNewestFirst(t *testing.T) {
	now := time.Unix(200, 0).UTC()
	service := newHistoryService(filepath.Join(t.TempDir(), "history.json"), historyServiceOptions{
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

func TestHistoryServiceDedupesTextAndMovesItToTop(t *testing.T) {
	now := time.Unix(200, 0).UTC()
	service := newHistoryService(filepath.Join(t.TempDir(), "history.json"), historyServiceOptions{
		Now: func() time.Time {
			return now
		},
	})

	first, err := service.AddText("same")
	if err != nil {
		t.Fatalf("add first: %v", err)
	}

	now = time.Unix(201, 0).UTC()
	if _, err := service.AddText("other"); err != nil {
		t.Fatalf("add other: %v", err)
	}

	now = time.Unix(202, 0).UTC()
	updated, err := service.AddText("same")
	if err != nil {
		t.Fatalf("add duplicate: %v", err)
	}

	items, err := service.List("")
	if err != nil {
		t.Fatalf("list history: %v", err)
	}

	if len(items) != 2 {
		t.Fatalf("expected duplicate to update existing item, got %d items", len(items))
	}
	if items[0].ID != first.ID || updated.ID != first.ID {
		t.Fatalf("expected duplicate to move original item to top, got updated=%#v list=%#v", updated, items)
	}
	if items[0].UseCount != 2 {
		t.Fatalf("expected use count 2, got %d", items[0].UseCount)
	}
}

func TestHistoryServiceSearchesTextCaseInsensitively(t *testing.T) {
	service := newHistoryService(filepath.Join(t.TempDir(), "history.json"), historyServiceOptions{
		Now: fixedClock(time.Unix(200, 0).UTC()),
	})

	if _, err := service.AddText("Unix timestamp 1719820800"); err != nil {
		t.Fatalf("add timestamp: %v", err)
	}
	if _, err := service.AddText("hello world"); err != nil {
		t.Fatalf("add hello: %v", err)
	}

	items, err := service.List("TIMESTAMP")
	if err != nil {
		t.Fatalf("search history: %v", err)
	}

	if len(items) != 1 || items[0].Text != "Unix timestamp 1719820800" {
		t.Fatalf("unexpected search result: %#v", items)
	}
}

func TestHistoryServiceDeletesAndClearsItems(t *testing.T) {
	service := newHistoryService(filepath.Join(t.TempDir(), "history.json"), historyServiceOptions{
		Now: fixedClock(time.Unix(200, 0).UTC()),
	})

	first, err := service.AddText("first")
	if err != nil {
		t.Fatalf("add first: %v", err)
	}
	if _, err := service.AddText("second"); err != nil {
		t.Fatalf("add second: %v", err)
	}

	if err := service.Delete(first.ID); err != nil {
		t.Fatalf("delete first: %v", err)
	}
	items, err := service.List("")
	if err != nil {
		t.Fatalf("list after delete: %v", err)
	}
	if len(items) != 1 || items[0].Text != "second" {
		t.Fatalf("expected only second after delete, got %#v", items)
	}

	if err := service.Clear(); err != nil {
		t.Fatalf("clear history: %v", err)
	}
	items, err = service.List("")
	if err != nil {
		t.Fatalf("list after clear: %v", err)
	}
	if len(items) != 0 {
		t.Fatalf("expected empty history after clear, got %#v", items)
	}
}

func TestHistoryServiceFindsItemByID(t *testing.T) {
	service := newHistoryService(filepath.Join(t.TempDir(), "history.json"), historyServiceOptions{
		Now: fixedClock(time.Unix(200, 0).UTC()),
	})

	item, err := service.AddText("copy me")
	if err != nil {
		t.Fatalf("add item: %v", err)
	}

	found, err := service.Get(item.ID)
	if err != nil {
		t.Fatalf("get item: %v", err)
	}

	if found.Text != "copy me" {
		t.Fatalf("expected copy me, got %#v", found)
	}
}
