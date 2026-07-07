package history

import "time"

type Record struct {
	ID         string    `json:"id"`
	Text       string    `json:"text"`
	Preview    string    `json:"preview"`
	Hash       string    `json:"hash"`
	CopiedAt   time.Time `json:"copiedAt"`
	LastUsedAt time.Time `json:"lastUsedAt"`
	UseCount   int       `json:"useCount"`
	Pinned     bool      `json:"pinned"`
}
