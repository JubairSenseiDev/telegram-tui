package main

import (
	"path/filepath"
	"testing"
)

func TestParseIDs(t *testing.T) {
	got := parseIDs("1, 2,,3")
	if len(got) != 3 || !got[1] || got[4] {
		t.Fatalf("parseIDs = %v, want 1,2,3", got)
	}
}

func TestParseChatID(t *testing.T) {
	if parseChatID("-100123456789") != -100123456789 {
		t.Fatal("negative chat id not parsed")
	}
	if parseChatID("") != 0 || parseChatID("abc") != 0 {
		t.Fatal("invalid chat id should parse to 0")
	}
}

func TestSubscribeDedup(t *testing.T) {
	a, err := newApp(config{DataDir: t.TempDir()})
	if err != nil {
		t.Fatal(err)
	}
	if !a.subscribe(1, 100, "One", "one") {
		t.Fatal("first subscribe should return new=true")
	}
	if a.subscribe(1, 100, "One", "one") {
		t.Fatal("duplicate subscribe should return new=false")
	}
	if len(a.sortedSubscribers()) != 1 {
		t.Fatalf("want 1 subscriber, got %d", len(a.sortedSubscribers()))
	}
	// updating chat id keeps single entry
	a.subscribe(1, 200, "One", "one")
	if len(a.sortedSubscribers()) != 1 {
		t.Fatalf("chat id update must not duplicate, got %d", len(a.sortedSubscribers()))
	}
}

func TestMatchKeyword(t *testing.T) {
	a := &app{keywords: map[string]string{"hello": "hi!", "price": "Check /shop"}}
	a.subscribers = []subscriber{}
	cases := []struct{ in, want string }{
		{"Hello world", "hi!"},
		{"hello", "hi!"},
		{"what is the PRICE?", "Check /shop"},
		{"nothing here", ""},
	}
	for _, c := range cases {
		if got := a.matchKeyword(c.in); got != c.want {
			t.Errorf("matchKeyword(%q) = %q, want %q", c.in, got, c.want)
		}
	}
}

func TestStateRoundtrip(t *testing.T) {
	dir := t.TempDir()
	a, err := newApp(config{DataDir: dir})
	if err != nil {
		t.Fatal(err)
	}
	a.subscribe(42, -100, "Ada", "ada")
	a.keywords["x"] = "y"
	_ = a.save("subscribers.json", a.subscribers)
	_ = a.save("keywords.json", a.keywords)

	b, err := newApp(config{DataDir: dir})
	if err != nil {
		t.Fatal(err)
	}
	if len(b.subscribers) != 1 || b.subscribers[0].ID != 42 {
		t.Fatalf("subscribers not persisted: %+v", b.subscribers)
	}
	if b.keywords["x"] != "y" {
		t.Fatalf("keywords not persisted: %v", b.keywords)
	}
	if filepath.Base(dir) == "" {
		t.Fatal("sanity")
	}
}
