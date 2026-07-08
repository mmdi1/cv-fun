package main

import (
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"
)

var debugLogMu sync.Mutex

func debugPrintf(format string, args ...any) {
	message := fmt.Sprintf(format, args...)
	fmt.Print(message)

	path, err := debugLogPath()
	if err != nil {
		return
	}

	debugLogMu.Lock()
	defer debugLogMu.Unlock()

	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return
	}

	line := fmt.Sprintf("%s %s", time.Now().Format(time.RFC3339), message)
	file, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o644)
	if err != nil {
		return
	}
	defer file.Close()

	_, _ = file.WriteString(line)
}

func debugLogPath() (string, error) {
	configDir, err := os.UserConfigDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(configDir, "cv-fun", "debug.log"), nil
}
