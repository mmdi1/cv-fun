package clipboard

import (
	"fmt"
	"strings"
	"testing"
	"time"

	"changeme/internal/history"
)

func TestMonitorWritesHistoryAndNotifies(t *testing.T) {
	now := time.Unix(100, 0).UTC()
	var records []history.Record
	notifications := 0
	monitor := NewMonitor(MonitorOptions{
		ReadClipboard: func() (string, bool) {
			return "history text", true
		},
		Logf: func(format string, args ...any) {},
		WriteHistory: func(record history.Record) error {
			records = append(records, record)
			return nil
		},
		OnHistoryChange: func() {
			notifications++
		},
		Delay:    0,
		Debounce: time.Millisecond,
		Now: func() time.Time {
			return now
		},
	})

	monitor.HandleCopyShortcut()

	if len(records) != 1 {
		t.Fatalf("expected 1 record, got %d: %#v", len(records), records)
	}
	if records[0].Text != "history text" {
		t.Fatalf("expected copied text in record, got %q", records[0].Text)
	}
	if notifications != 1 {
		t.Fatalf("expected 1 notification, got %d", notifications)
	}
}

func TestMonitorDebouncesDuplicateShortcutEvents(t *testing.T) {
	var logs []string
	now := time.Unix(100, 0)
	monitor := NewMonitor(MonitorOptions{
		ReadClipboard: func() (string, bool) {
			return "same copy", true
		},
		Logf: func(format string, args ...any) {
			logs = append(logs, fmt.Sprintf(format, args...))
		},
		Delay:    0,
		Debounce: 100 * time.Millisecond,
		Now: func() time.Time {
			return now
		},
	})

	monitor.HandleCopyShortcut()
	now = now.Add(50 * time.Millisecond)
	monitor.HandleCopyShortcut()

	if len(logs) != 1 {
		t.Fatalf("expected duplicate event to be ignored, got %d logs: %#v", len(logs), logs)
	}
}

func TestMonitorLogsUnavailableClipboardText(t *testing.T) {
	var logs []string
	monitor := NewMonitor(MonitorOptions{
		ReadClipboard: func() (string, bool) {
			return "", false
		},
		Logf: func(format string, args ...any) {
			logs = append(logs, fmt.Sprintf(format, args...))
		},
		Delay:    0,
		Debounce: time.Millisecond,
		Now: func() time.Time {
			return time.Unix(100, 0)
		},
	})

	monitor.HandleCopyShortcut()

	if len(logs) != 1 || !strings.Contains(logs[0], "clipboard text unavailable") {
		t.Fatalf("expected unavailable clipboard log, got %#v", logs)
	}
}
