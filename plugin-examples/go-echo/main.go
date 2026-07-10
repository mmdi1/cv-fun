package main

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
)

type input struct {
	Content string `json:"content"`
	Type    string `json:"type"`
}

type output struct {
	OK      bool   `json:"ok"`
	Title   string `json:"title"`
	Body    string `json:"body"`
	Preview string `json:"preview"`
	Hint    string `json:"hint"`
	Error   string `json:"error,omitempty"`
}

func main() {
	raw, err := io.ReadAll(os.Stdin)
	if err != nil {
		emit(output{OK: false, Error: err.Error()})
		return
	}
	var in input
	if err := json.Unmarshal(raw, &in); err != nil {
		emit(output{OK: false, Error: "invalid json: " + err.Error()})
		return
	}
	body := fmt.Sprintf("[go-echo] type=%s\n%s", in.Type, in.Content)
	preview := in.Content
	if len(preview) > 72 {
		preview = preview[:72] + "…"
	}
	emit(output{
		OK:      true,
		Title:   "Go 回显",
		Body:    body,
		Preview: preview,
		Hint:    "go",
	})
}

func emit(o output) {
	_ = json.NewEncoder(os.Stdout).Encode(o)
}
