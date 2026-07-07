package main

import "changeme/internal/history"

type historyServiceOptions = history.ServiceOptions
type historyService = history.Service

func newHistoryService(path string, options historyServiceOptions) *historyService {
	return history.NewService(path, options)
}

func sortHistory(records []clipboardRecord) []clipboardRecord {
	return history.Sort(records)
}

func previewText(text string) string {
	return history.PreviewText(text)
}

func newHistoryID() string {
	return history.NewID()
}
