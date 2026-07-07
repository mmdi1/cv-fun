package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

func TestDefaultAppConfigMatchesPlannedFirstSettings(t *testing.T) {
	config := defaultAppConfig()

	if config.History.MaxItems != 500 {
		t.Fatalf("expected default history max items 500, got %d", config.History.MaxItems)
	}
	if config.Hotkeys.OpenPanel != "CmdOrCtrl+Shift+C" {
		t.Fatalf("expected default open panel hotkey, got %q", config.Hotkeys.OpenPanel)
	}
	if config.Hotkeys.PasteLatest != "CmdOrCtrl+Shift+V" {
		t.Fatalf("expected default paste latest hotkey, got %q", config.Hotkeys.PasteLatest)
	}
	if config.Hotkeys.CopyShortcut != "CmdOrCtrl+C" {
		t.Fatalf("expected default copy shortcut hotkey, got %q", config.Hotkeys.CopyShortcut)
	}
	if !config.Parsers.Enabled["timestamp"] || !config.Parsers.Enabled["json"] || !config.Parsers.Enabled["url"] {
		t.Fatalf("expected core parsers enabled by default: %#v", config.Parsers.Enabled)
	}
	if config.Translation.Provider != "disabled" {
		t.Fatalf("expected disabled translation provider by default, got %q", config.Translation.Provider)
	}
}

func TestConfigStoreLoadReturnsDefaultWhenFileDoesNotExist(t *testing.T) {
	store := newConfigStore(filepath.Join(t.TempDir(), "config.json"))

	config, err := store.Load()
	if err != nil {
		t.Fatalf("load missing config: %v", err)
	}

	if config.History.MaxItems != 500 {
		t.Fatalf("expected default max items, got %d", config.History.MaxItems)
	}
}

func TestConfigStoreSaveAndLoadRoundTripsUserConfig(t *testing.T) {
	store := newConfigStore(filepath.Join(t.TempDir(), "config.json"))
	config := defaultAppConfig()
	config.History.MaxItems = 120
	config.Hotkeys.OpenPanel = "CmdOrCtrl+Alt+C"
	config.Hotkeys.CopyShortcut = "Command+Option+C"
	config.Parsers.Enabled["wordTranslation"] = false
	config.Translation.Provider = "local"

	saved, err := store.Save(config)
	if err != nil {
		t.Fatalf("save config: %v", err)
	}
	loaded, err := store.Load()
	if err != nil {
		t.Fatalf("load config: %v", err)
	}

	if saved.History.MaxItems != 120 || loaded.History.MaxItems != 120 {
		t.Fatalf("expected max items round trip, saved=%d loaded=%d", saved.History.MaxItems, loaded.History.MaxItems)
	}
	if loaded.Hotkeys.OpenPanel != "CmdOrCtrl+Alt+C" {
		t.Fatalf("expected custom open panel hotkey, got %q", loaded.Hotkeys.OpenPanel)
	}
	if loaded.Hotkeys.CopyShortcut != "Command+Option+C" {
		t.Fatalf("expected custom copy shortcut hotkey, got %q", loaded.Hotkeys.CopyShortcut)
	}
	if loaded.Parsers.Enabled["wordTranslation"] {
		t.Fatalf("expected word translation parser disabled after load")
	}
	if loaded.Translation.Provider != "local" {
		t.Fatalf("expected local translation provider, got %q", loaded.Translation.Provider)
	}
}

func TestConfigStoreLoadMergesMissingFieldsWithDefaults(t *testing.T) {
	path := filepath.Join(t.TempDir(), "config.json")
	content := []byte(`{"history":{"maxItems":64},"parsers":{"enabled":{"json":false}}}` + "\n")
	if err := os.WriteFile(path, content, 0o644); err != nil {
		t.Fatalf("write partial config: %v", err)
	}

	config, err := newConfigStore(path).Load()
	if err != nil {
		t.Fatalf("load partial config: %v", err)
	}

	if config.History.MaxItems != 64 {
		t.Fatalf("expected configured max items, got %d", config.History.MaxItems)
	}
	if config.Hotkeys.OpenPanel != "CmdOrCtrl+Shift+C" {
		t.Fatalf("expected default open panel hotkey, got %q", config.Hotkeys.OpenPanel)
	}
	if config.Hotkeys.CopyShortcut != "CmdOrCtrl+C" {
		t.Fatalf("expected default copy shortcut hotkey, got %q", config.Hotkeys.CopyShortcut)
	}
	if config.Parsers.Enabled["json"] {
		t.Fatalf("expected explicit json=false to be preserved")
	}
	if !config.Parsers.Enabled["timestamp"] {
		t.Fatalf("expected missing timestamp parser to default true")
	}
}

func TestConfigServiceExposesLoadAndSave(t *testing.T) {
	service := NewConfigService(newConfigStore(filepath.Join(t.TempDir(), "config.json")))
	config, err := service.GetConfig()
	if err != nil {
		t.Fatalf("get config: %v", err)
	}

	config.History.MaxItems = 42
	if _, err := service.SaveConfig(config); err != nil {
		t.Fatalf("save config through service: %v", err)
	}
	next, err := service.GetConfig()
	if err != nil {
		t.Fatalf("get saved config: %v", err)
	}

	if next.History.MaxItems != 42 {
		t.Fatalf("expected saved max items 42, got %d", next.History.MaxItems)
	}
}

func TestAppConfigWritesReadableJSON(t *testing.T) {
	path := filepath.Join(t.TempDir(), "nested", "config.json")
	if _, err := newConfigStore(path).Save(defaultAppConfig()); err != nil {
		t.Fatalf("save config: %v", err)
	}

	content, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read config: %v", err)
	}

	var decoded map[string]any
	if err := json.Unmarshal(content, &decoded); err != nil {
		t.Fatalf("config should be valid json: %v\n%s", err, string(content))
	}
	if _, ok := decoded["history"]; !ok {
		t.Fatalf("expected history section in config json: %s", string(content))
	}
}
