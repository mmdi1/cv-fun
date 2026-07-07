package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestCopyMonitorLogsClipboardTextAfterDelay(t *testing.T) {
	var logs []string
	monitor := newCopyMonitor(copyMonitorOptions{
		ReadClipboard: func() (string, bool) {
			return "hello clipboard", true
		},
		Logf: func(format string, args ...any) {
			logs = append(logs, fmt.Sprintf(format, args...))
		},
		Delay:    0,
		Debounce: time.Millisecond,
		Now:      fixedClock(time.Unix(100, 0)),
	})

	monitor.HandleCopyShortcut()

	if len(logs) != 1 {
		t.Fatalf("expected 1 log entry, got %d: %#v", len(logs), logs)
	}
	if !strings.Contains(logs[0], "hello clipboard") {
		t.Fatalf("expected copied text in log, got %q", logs[0])
	}
}

func TestCopyMonitorLogsUnavailableWhenClipboardHasNoText(t *testing.T) {
	var logs []string
	monitor := newCopyMonitor(copyMonitorOptions{
		ReadClipboard: func() (string, bool) {
			return "", false
		},
		Logf: func(format string, args ...any) {
			logs = append(logs, fmt.Sprintf(format, args...))
		},
		Delay:    0,
		Debounce: time.Millisecond,
		Now:      fixedClock(time.Unix(100, 0)),
	})

	monitor.HandleCopyShortcut()

	if len(logs) != 1 {
		t.Fatalf("expected 1 log entry, got %d: %#v", len(logs), logs)
	}
	if !strings.Contains(logs[0], "clipboard text unavailable") {
		t.Fatalf("expected unavailable clipboard log, got %q", logs[0])
	}
}

func TestCopyMonitorDebouncesDuplicateShortcutEvents(t *testing.T) {
	var logs []string
	now := time.Unix(100, 0)
	monitor := newCopyMonitor(copyMonitorOptions{
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

func TestCopyMonitorWritesClipboardHistory(t *testing.T) {
	var records []clipboardRecord
	now := time.Unix(100, 0).UTC()
	monitor := newCopyMonitor(copyMonitorOptions{
		ReadClipboard: func() (string, bool) {
			return "history text", true
		},
		Logf: func(format string, args ...any) {},
		WriteHistory: func(record clipboardRecord) error {
			records = append(records, record)
			return nil
		},
		Delay:    0,
		Debounce: time.Millisecond,
		Now:      fixedClock(now),
	})

	monitor.HandleCopyShortcut()

	if len(records) != 1 {
		t.Fatalf("expected 1 history record, got %d: %#v", len(records), records)
	}
	if records[0].Text != "history text" {
		t.Fatalf("expected copied text in history, got %q", records[0].Text)
	}
	if !records[0].CopiedAt.Equal(now) {
		t.Fatalf("expected copied time %s, got %s", now, records[0].CopiedAt)
	}
}

func TestCopyMonitorNotifiesAfterHistoryWrite(t *testing.T) {
	notifications := 0
	monitor := newCopyMonitor(copyMonitorOptions{
		ReadClipboard: func() (string, bool) {
			return "history text", true
		},
		Logf: func(format string, args ...any) {},
		WriteHistory: func(record clipboardRecord) error {
			return nil
		},
		OnHistoryChange: func() {
			notifications++
		},
		Delay:    0,
		Debounce: time.Millisecond,
		Now:      fixedClock(time.Unix(100, 0)),
	})

	monitor.HandleCopyShortcut()

	if notifications != 1 {
		t.Fatalf("expected 1 history change notification, got %d", notifications)
	}
}

func TestCopyMonitorDoesNotNotifyWhenHistoryWriteFails(t *testing.T) {
	notifications := 0
	monitor := newCopyMonitor(copyMonitorOptions{
		ReadClipboard: func() (string, bool) {
			return "history text", true
		},
		Logf: func(format string, args ...any) {},
		WriteHistory: func(record clipboardRecord) error {
			return fmt.Errorf("write failed")
		},
		OnHistoryChange: func() {
			notifications++
		},
		Delay:    0,
		Debounce: time.Millisecond,
		Now:      fixedClock(time.Unix(100, 0)),
	})

	monitor.HandleCopyShortcut()

	if notifications != 0 {
		t.Fatalf("expected no notification after failed history write, got %d", notifications)
	}
}

func TestWriteClipboardHistoryAppendsJSONRecords(t *testing.T) {
	path := filepath.Join(t.TempDir(), "history", "clipboard.json")

	first := clipboardRecord{CopiedAt: time.Unix(100, 0).UTC(), Text: "first"}
	second := clipboardRecord{CopiedAt: time.Unix(101, 0).UTC(), Text: "second"}

	if err := writeClipboardHistory(path, first); err != nil {
		t.Fatalf("write first history record: %v", err)
	}
	if err := writeClipboardHistory(path, second); err != nil {
		t.Fatalf("write second history record: %v", err)
	}

	content, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read history file: %v", err)
	}

	var records []clipboardRecord
	if err := json.Unmarshal(content, &records); err != nil {
		t.Fatalf("history file should contain a JSON array: %v\n%s", err, string(content))
	}

	if len(records) != 2 {
		t.Fatalf("expected 2 records, got %d: %#v", len(records), records)
	}
	if records[0].Text != "first" || records[1].Text != "second" {
		t.Fatalf("unexpected history records: %#v", records)
	}
}

func fixedClock(t time.Time) func() time.Time {
	return func() time.Time {
		return t
	}
}
