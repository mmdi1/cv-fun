package main

import "changeme/internal/clipboard"

type copyMonitorOptions = clipboard.MonitorOptions
type copyMonitor = clipboard.Monitor

func newCopyMonitor(options copyMonitorOptions) *copyMonitor {
	return clipboard.NewMonitor(options)
}
