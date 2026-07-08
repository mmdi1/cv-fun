package main

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"sync"
)

type appConfig struct {
	History     historyConfig     `json:"history"`
	Hotkeys     hotkeyConfig      `json:"hotkeys"`
	Parsers     parserConfig      `json:"parsers"`
	Translation translationConfig `json:"translation"`
}

type historyConfig struct {
	MaxItems int `json:"maxItems"`
}

type hotkeyConfig struct {
	OpenPanel    string `json:"openPanel"`
	PasteLatest  string `json:"pasteLatest"`
	CopyShortcut string `json:"copyShortcut"`
}

type parserConfig struct {
	Enabled map[string]bool `json:"enabled"`
}

type translationConfig struct {
	Provider       string `json:"provider"`
	TargetLanguage string `json:"targetLanguage"`
}

type configStore struct {
	path string
	mu   sync.Mutex
}

type ConfigService struct {
	store *configStore
}

func defaultAppConfig() appConfig {
	return appConfig{
		History: historyConfig{
			MaxItems: 500,
		},
		Hotkeys: hotkeyConfig{
			OpenPanel:    "CmdOrCtrl+Shift+C",
			PasteLatest:  "CmdOrCtrl+Shift+V",
			CopyShortcut: "CmdOrCtrl+C",
		},
		Parsers: parserConfig{
			Enabled: map[string]bool{
				"timestamp":       true,
				"datetime":        true,
				"wordTranslation": true,
				"json":            true,
				"url":             true,
				"base64":          true,
				"color":           true,
				"uuid":            true,
			},
		},
		Translation: translationConfig{
			Provider:       "disabled",
			TargetLanguage: "zh-CN",
		},
	}
}

func appConfigPath() (string, error) {
	configDir, err := os.UserConfigDir()
	if err != nil {
		return "", err
	}

	return filepath.Join(configDir, "cv-fun", "config.json"), nil
}

func newConfigStore(path string) *configStore {
	return &configStore{path: path}
}

func (s *configStore) Load() (appConfig, error) {
	if s == nil || s.path == "" {
		return appConfig{}, errors.New("config path is empty")
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	content, err := os.ReadFile(s.path)
	if errors.Is(err, os.ErrNotExist) {
		return defaultAppConfig(), nil
	}
	if err != nil {
		return appConfig{}, err
	}
	if len(content) == 0 {
		return defaultAppConfig(), nil
	}

	config := defaultAppConfig()
	if err := json.Unmarshal(content, &config); err != nil {
		return appConfig{}, err
	}

	return normalizeAppConfig(config), nil
}

func (s *configStore) Save(config appConfig) (appConfig, error) {
	if s == nil || s.path == "" {
		return appConfig{}, errors.New("config path is empty")
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	config = normalizeAppConfig(config)
	if err := os.MkdirAll(filepath.Dir(s.path), 0o755); err != nil {
		return appConfig{}, err
	}

	content, err := json.MarshalIndent(config, "", "  ")
	if err != nil {
		return appConfig{}, err
	}
	content = append(content, '\n')

	if err := os.WriteFile(s.path, content, 0o644); err != nil {
		return appConfig{}, err
	}

	return config, nil
}

func normalizeAppConfig(config appConfig) appConfig {
	defaults := defaultAppConfig()

	if config.History.MaxItems <= 0 {
		config.History.MaxItems = defaults.History.MaxItems
	}
	if config.Hotkeys.OpenPanel == "" {
		config.Hotkeys.OpenPanel = defaults.Hotkeys.OpenPanel
	}
	if config.Hotkeys.PasteLatest == "" {
		config.Hotkeys.PasteLatest = defaults.Hotkeys.PasteLatest
	}
	if config.Hotkeys.CopyShortcut == "" {
		config.Hotkeys.CopyShortcut = defaults.Hotkeys.CopyShortcut
	}
	if config.Parsers.Enabled == nil {
		config.Parsers.Enabled = map[string]bool{}
	}
	for key, value := range defaults.Parsers.Enabled {
		if _, ok := config.Parsers.Enabled[key]; !ok {
			config.Parsers.Enabled[key] = value
		}
	}
	if config.Translation.Provider == "" {
		config.Translation.Provider = defaults.Translation.Provider
	}
	if config.Translation.TargetLanguage == "" {
		config.Translation.TargetLanguage = defaults.Translation.TargetLanguage
	}

	return config
}

func NewConfigService(store *configStore) *ConfigService {
	return &ConfigService{store: store}
}

func (s *ConfigService) GetConfig() (appConfig, error) {
	return s.store.Load()
}

func (s *ConfigService) SaveConfig(config appConfig) (appConfig, error) {
	return s.store.Save(config)
}
