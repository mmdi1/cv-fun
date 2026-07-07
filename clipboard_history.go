package main

import "changeme/internal/history"

type clipboardRecord = history.Record

func clipboardHistoryPath() (string, error) {
	return history.DefaultPath()
}

func writeClipboardHistory(path string, record clipboardRecord) error {
	return history.WriteJSON(path, record)
}

func hashClipboardText(text string) string {
	return history.HashText(text)
}
