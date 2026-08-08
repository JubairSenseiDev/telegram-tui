package main

import (
	"encoding/json"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"sort"
	"sync"
	"time"

	"github.com/PaulSonOfLars/gotgbot/v2"
)

type subscriber struct {
	ID       int64  `json:"id"`
	ChatID   int64  `json:"chat_id"`
	Name     string `json:"name"`
	Username string `json:"username"`
	Added    int64  `json:"added"`
}

type scheduleItem struct {
	ID       int64  `json:"id"`
	ChatID   int64  `json:"chat_id"`
	Text     string `json:"text"`
	Interval int64  `json:"interval"`
	Last     int64  `json:"last"`
}

type seenChat struct {
	ID        int64  `json:"id"`
	ChatID    int64  `json:"chat_id"`
	Name      string `json:"name"`
	Username  string `json:"username"`
	FirstSeen int64  `json:"first_seen"`
	LastSeen  int64  `json:"last_seen"`
}

type app struct {
	bot *gotgbot.Bot
	cfg config

	mu          sync.Mutex
	subscribers []subscriber
	keywords    map[string]string
	schedules   []scheduleItem
	seen        map[int64]seenChat
	nextID      int64
}

func newApp(cfg config) (*app, error) {
	if err := os.MkdirAll(cfg.DataDir, 0o700); err != nil {
		return nil, fmt.Errorf("mkdir %s: %w", cfg.DataDir, err)
	}
	a := &app{
		cfg:         cfg,
		keywords:    map[string]string{},
		subscribers: []subscriber{},
		seen:        map[int64]seenChat{},
	}
	if err := a.load(); err != nil {
		return nil, err
	}
	return a, nil
}

func (a *app) path(name string) string {
	return filepath.Join(a.cfg.DataDir, name)
}

func (a *app) load() error {
	a.subscribers = loadJSON[[]subscriber](a.path("subscribers.json"), a.subscribers)
	a.keywords = loadJSON[map[string]string](a.path("keywords.json"), a.keywords)
	a.schedules = loadJSON[[]scheduleItem](a.path("schedules.json"), a.schedules)
	a.seen = loadJSON[map[int64]seenChat](a.path("seen.json"), a.seen)
	for _, s := range a.schedules {
		if s.ID >= a.nextID {
			a.nextID = s.ID + 1
		}
	}
	return nil
}

func loadJSON[T any](path string, fallback T) T {
	data, err := os.ReadFile(path)
	if err != nil {
		return fallback
	}
	var out T
	if err := json.Unmarshal(data, &out); err != nil {
		log.Printf("corrupt %s: %v", path, err)
		return fallback
	}
	return out
}

func (a *app) save(key string, v any) error {
	data, err := json.MarshalIndent(v, "", "  ")
	if err != nil {
		return err
	}
	tmp := a.path(key + ".tmp")
	if err := os.WriteFile(tmp, data, 0o600); err != nil {
		return err
	}
	return os.Rename(tmp, a.path(key))
}

func (a *app) saveAll() {
	a.mu.Lock()
	defer a.mu.Unlock()
	_ = a.save("subscribers.json", a.subscribers)
	_ = a.save("keywords.json", a.keywords)
	_ = a.save("schedules.json", a.schedules)
	_ = a.save("seen.json", a.seen)
}

func (a *app) isAdmin(userID int64) bool {
	return a.cfg.AdminIDs[userID]
}

// subscribe registers a chat (private or group) for broadcasts.
func (a *app) subscribe(uid, chatID int64, name, username string) bool {
	a.mu.Lock()
	defer a.mu.Unlock()
	for i := range a.subscribers {
		if a.subscribers[i].ID == uid {
			if a.subscribers[i].ChatID != chatID {
				a.subscribers[i].ChatID = chatID
				_ = a.save("subscribers.json", a.subscribers)
			}
			return false
		}
	}
	a.subscribers = append(a.subscribers, subscriber{
		ID: uid, ChatID: chatID, Name: name, Username: username, Added: time.Now().Unix(),
	})
	_ = a.save("subscribers.json", a.subscribers)
	return true
}

func (a *app) sortedSubscribers() []subscriber {
	a.mu.Lock()
	defer a.mu.Unlock()
	out := append([]subscriber(nil), a.subscribers...)
	sort.Slice(out, func(i, j int) bool { return out[i].Added < out[j].Added })
	return out
}

// recordSeen remembers every user/chat that ever contacts the bot.
func (a *app) recordSeen(uid, chatID int64, name, username string) {
	if uid == 0 {
		return
	}
	now := time.Now().Unix()
	a.mu.Lock()
	sc, ok := a.seen[uid]
	if !ok {
		sc = seenChat{ID: uid, ChatID: chatID, FirstSeen: now}
	}
	sc.LastSeen = now
	if chatID != 0 {
		sc.ChatID = chatID
	}
	if name != "" {
		sc.Name = name
	}
	if username != "" {
		sc.Username = username
	}
	a.seen[uid] = sc
	a.mu.Unlock()
}

func (a *app) sortedSeen() []seenChat {
	a.mu.Lock()
	defer a.mu.Unlock()
	out := make([]seenChat, 0, len(a.seen))
	for _, sc := range a.seen {
		out = append(out, sc)
	}
	sort.Slice(out, func(i, j int) bool { return out[i].FirstSeen < out[j].FirstSeen })
	return out
}
