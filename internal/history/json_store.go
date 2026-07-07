package history

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
)

func WriteJSON(path string, record Record) error {
	if path == "" {
		return errors.New("clipboard history path is empty")
	}

	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}

	records := make([]Record, 0)
	content, err := os.ReadFile(path)
	if err == nil && len(content) > 0 {
		if err := json.Unmarshal(content, &records); err != nil {
			return err
		}
	} else if err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}

	records = append(records, record)

	nextContent, err := json.MarshalIndent(records, "", "  ")
	if err != nil {
		return err
	}
	nextContent = append(nextContent, '\n')

	return os.WriteFile(path, nextContent, 0o644)
}

func HashText(text string) string {
	sum := sha256.Sum256([]byte(text))
	return hex.EncodeToString(sum[:])
}
