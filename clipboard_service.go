package main

type ClipboardService struct {
	history        *historyService
	writeClipboard func(text string) bool
}

func NewClipboardService(history *historyService) *ClipboardService {
	return &ClipboardService{history: history}
}

func (s *ClipboardService) setClipboardWriter(writeClipboard func(text string) bool) {
	s.writeClipboard = writeClipboard
}

func (s *ClipboardService) ListHistory(query string) ([]clipboardRecord, error) {
	if s.history == nil {
		return []clipboardRecord{}, nil
	}

	return s.history.List(query)
}

func (s *ClipboardService) DeleteHistory(id string) error {
	if s.history == nil {
		return nil
	}

	return s.history.Delete(id)
}

func (s *ClipboardService) ClearHistory() error {
	if s.history == nil {
		return nil
	}

	return s.history.Clear()
}

func (s *ClipboardService) CopyHistory(id string) error {
	if s.history == nil || s.writeClipboard == nil {
		return nil
	}

	record, err := s.history.Get(id)
	if err != nil {
		return err
	}

	s.writeClipboard(record.Text)
	return nil
}
