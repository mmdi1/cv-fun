package history

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"
)

type ServiceOptions struct {
	Now      func() time.Time
	MaxItems int
}

type Service struct {
	path     string
	now      func() time.Time
	maxItems int
	mu       sync.Mutex
}

func NewService(path string, options ServiceOptions) *Service {
	now := options.Now
	if now == nil {
		now = time.Now
	}
	maxItems := options.MaxItems
	if maxItems <= 0 {
		maxItems = 500
	}

	return &Service{
		path:     path,
		now:      now,
		maxItems: maxItems,
	}
}

func (s *Service) AddText(text string) (Record, error) {
	text = strings.TrimSpace(text)
	if text == "" {
		return Record{}, errors.New("clipboard text is empty")
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	records, err := s.load()
	if err != nil {
		return Record{}, err
	}

	now := s.now().UTC()
	hash := HashText(text)
	for index := range records {
		if records[index].Hash == hash || records[index].Text == text {
			records[index].Hash = hash
			records[index].Text = text
			records[index].Preview = PreviewText(text)
			records[index].CopiedAt = now
			records[index].LastUsedAt = now
			records[index].UseCount++
			if records[index].UseCount <= 0 {
				records[index].UseCount = 1
			}
			record := records[index]
			if err := s.save(records); err != nil {
				return Record{}, err
			}
			return record, nil
		}
	}

	record := Record{
		ID:         NewID(),
		Text:       text,
		Preview:    PreviewText(text),
		Hash:       hash,
		CopiedAt:   now,
		LastUsedAt: now,
		UseCount:   1,
	}
	records = append(records, record)
	if err := s.save(records); err != nil {
		return Record{}, err
	}

	return record, nil
}

func (s *Service) List(query string) ([]Record, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	records, err := s.load()
	if err != nil {
		return nil, err
	}
	records = Sort(records)

	query = strings.TrimSpace(strings.ToLower(query))
	if query == "" {
		return records, nil
	}

	result := make([]Record, 0, len(records))
	for _, record := range records {
		if strings.Contains(strings.ToLower(record.Text), query) || strings.Contains(strings.ToLower(record.Preview), query) {
			result = append(result, record)
		}
	}

	return result, nil
}

func (s *Service) Delete(id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	records, err := s.load()
	if err != nil {
		return err
	}

	next := make([]Record, 0, len(records))
	for _, record := range records {
		if record.ID != id {
			next = append(next, record)
		}
	}

	return s.save(next)
}

func (s *Service) Get(id string) (Record, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	records, err := s.load()
	if err != nil {
		return Record{}, err
	}

	for _, record := range records {
		if record.ID == id {
			return record, nil
		}
	}

	return Record{}, errors.New("history item not found")
}

func (s *Service) Clear() error {
	s.mu.Lock()
	defer s.mu.Unlock()

	return s.save([]Record{})
}

func (s *Service) load() ([]Record, error) {
	if s.path == "" {
		return nil, errors.New("history path is empty")
	}

	content, err := os.ReadFile(s.path)
	if errors.Is(err, os.ErrNotExist) {
		return []Record{}, nil
	}
	if err != nil {
		return nil, err
	}
	if len(content) == 0 {
		return []Record{}, nil
	}

	var records []Record
	if err := json.Unmarshal(content, &records); err != nil {
		return nil, err
	}

	changed := false
	for index := range records {
		if records[index].ID == "" {
			records[index].ID = NewID()
			changed = true
		}
		if records[index].Hash == "" && records[index].Text != "" {
			records[index].Hash = HashText(records[index].Text)
			changed = true
		}
		if records[index].Preview == "" {
			records[index].Preview = PreviewText(records[index].Text)
			changed = true
		}
		if records[index].UseCount <= 0 {
			records[index].UseCount = 1
			changed = true
		}
		if records[index].LastUsedAt.IsZero() {
			records[index].LastUsedAt = records[index].CopiedAt
			changed = true
		}
	}
	if changed {
		if err := s.save(records); err != nil {
			return nil, err
		}
	}

	return records, nil
}

func (s *Service) save(records []Record) error {
	if s.path == "" {
		return errors.New("history path is empty")
	}

	records = Sort(records)
	if len(records) > s.maxItems {
		records = records[:s.maxItems]
	}

	if err := os.MkdirAll(filepath.Dir(s.path), 0o755); err != nil {
		return err
	}

	content, err := json.MarshalIndent(records, "", "  ")
	if err != nil {
		return err
	}
	content = append(content, '\n')

	return os.WriteFile(s.path, content, 0o644)
}

func Sort(records []Record) []Record {
	next := append([]Record(nil), records...)
	sort.SliceStable(next, func(i, j int) bool {
		if next[i].Pinned != next[j].Pinned {
			return next[i].Pinned
		}
		return next[i].CopiedAt.After(next[j].CopiedAt)
	})
	return next
}

func PreviewText(text string) string {
	text = strings.Join(strings.Fields(text), " ")
	if len([]rune(text)) <= 120 {
		return text
	}

	runes := []rune(text)
	return string(runes[:120]) + "..."
}

func NewID() string {
	var bytes [16]byte
	if _, err := rand.Read(bytes[:]); err != nil {
		return hex.EncodeToString([]byte(time.Now().UTC().Format(time.RFC3339Nano)))
	}

	bytes[6] = (bytes[6] & 0x0f) | 0x40
	bytes[8] = (bytes[8] & 0x3f) | 0x80
	return hex.EncodeToString(bytes[:])
}
