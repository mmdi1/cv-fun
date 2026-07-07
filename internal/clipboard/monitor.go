package clipboard

import (
	"log"
	"sync"
	"time"

	"changeme/internal/history"
)

type MonitorOptions struct {
	ReadClipboard   func() (string, bool)
	Logf            func(format string, args ...any)
	WriteHistory    func(record history.Record) error
	OnHistoryChange func()
	Delay           time.Duration
	Debounce        time.Duration
	Now             func() time.Time
}

type Monitor struct {
	readClipboard   func() (string, bool)
	logf            func(format string, args ...any)
	writeHistory    func(record history.Record) error
	onHistoryChange func()
	delay           time.Duration
	debounce        time.Duration
	now             func() time.Time

	mu        sync.Mutex
	lastEvent time.Time
}

func NewMonitor(options MonitorOptions) *Monitor {
	monitor := &Monitor{
		readClipboard:   options.ReadClipboard,
		logf:            options.Logf,
		writeHistory:    options.WriteHistory,
		onHistoryChange: options.OnHistoryChange,
		delay:           options.Delay,
		debounce:        options.Debounce,
		now:             options.Now,
	}

	if monitor.logf == nil {
		monitor.logf = log.Printf
	}
	if monitor.now == nil {
		monitor.now = time.Now
	}
	if monitor.debounce <= 0 {
		monitor.debounce = 100 * time.Millisecond
	}

	return monitor
}

func (m *Monitor) HandleCopyShortcut() {
	if !m.acceptEvent() {
		return
	}

	if m.delay > 0 {
		time.Sleep(m.delay)
	}

	if m.readClipboard == nil {
		m.logf("clipboard text unavailable")
		return
	}

	text, ok := m.readClipboard()
	if !ok {
		m.logf("clipboard text unavailable")
		return
	}

	m.HandleCopiedText(text)
}

func (m *Monitor) HandleCopiedText(text string) {
	m.logf("copied text: %s", text)
	if m.writeHistory != nil {
		if err := m.writeHistory(history.Record{CopiedAt: m.now().UTC(), Text: text}); err != nil {
			m.logf("failed to write clipboard history: %v", err)
			return
		}
		if m.onHistoryChange != nil {
			m.onHistoryChange()
		}
	}
}

func (m *Monitor) acceptEvent() bool {
	now := m.now()

	m.mu.Lock()
	defer m.mu.Unlock()

	if !m.lastEvent.IsZero() && now.Sub(m.lastEvent) < m.debounce {
		return false
	}

	m.lastEvent = now
	return true
}
