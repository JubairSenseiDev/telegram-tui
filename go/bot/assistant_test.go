package main

import (
	"testing"

	"github.com/PaulSonOfLars/gotgbot/v2"
)

func TestMediaOf(t *testing.T) {
	mt, ext, fid, size := mediaOf(&gotgbot.Message{
		Video: &gotgbot.Video{FileId: "VID1", FileSize: 1000},
	})
	if mt != "video" || ext != "mp4" || fid != "VID1" || size != 1000 {
		t.Fatalf("video mediaOf = %q %q %q %d", mt, ext, fid, size)
	}

	mt, ext, _, _ = mediaOf(&gotgbot.Message{
		Photo: []gotgbot.PhotoSize{{FileId: "small", FileSize: 10}, {FileId: "big", FileSize: 99}},
	})
	if mt != "photo" || ext != "jpg" {
		t.Fatalf("photo mediaOf = %q %q", mt, ext)
	}

	mt, ext, _, _ = mediaOf(&gotgbot.Message{
		Document: &gotgbot.Document{FileId: "DOC1", FileSize: 5, FileName: "report.pdf"},
	})
	if mt != "document" || ext != "pdf" {
		t.Fatalf("document mediaOf = %q %q", mt, ext)
	}

	if mt, _, _, _ := mediaOf(&gotgbot.Message{}); mt != "" {
		t.Fatalf("empty media should be empty, got %q", mt)
	}
}

func TestTmeLink(t *testing.T) {
	cases := []struct {
		chatID, msgID int64
		want          string
	}{
		{-1001550117445, 42, "https://t.me/c/1550117445/42"},
		{-1002000000010, 7, "https://t.me/c/2000000010/7"},
		{12345, 9, "https://t.me/c/12345/9"},
	}
	for _, c := range cases {
		if got := tmeLink(c.chatID, c.msgID); got != c.want {
			t.Errorf("tmeLink(%d,%d) = %q, want %q", c.chatID, c.msgID, got, c.want)
		}
	}
}

func TestLoadPostsRoundtrip(t *testing.T) {
	a, err := newApp(config{DataDir: t.TempDir()})
	if err != nil {
		t.Fatal(err)
	}
	a.assistNext = 1
	p := &savedPost{ID: 1, ChatID: -100123, MessageID: 5, Text: "hello", MediaType: "video", FileID: "F"}
	a.savePost(nil, p, "-100123:5")

	b, err := newApp(config{DataDir: a.cfg.DataDir})
	if err != nil {
		t.Fatal(err)
	}
	if len(b.posts) != 1 || b.posts[0].ID != 1 || b.posts[0].Text != "hello" {
		t.Fatalf("posts not reloaded: %+v", b.posts)
	}
	if id, ok := b.postIdx["-100123:5"]; !ok || id != 1 {
		t.Fatalf("post index not reloaded: %v", b.postIdx)
	}
	if b.assistNext != 2 {
		t.Fatalf("assistNext = %d, want 2", b.assistNext)
	}
	// saving the same key again must update, not duplicate
	b.savePost(nil, &savedPost{ChatID: -100123, MessageID: 5, Text: "edited"}, "-100123:5")
	if len(b.posts) != 1 || b.posts[0].Text != "edited" {
		t.Fatalf("edit must not duplicate posts: %+v", b.posts)
	}
}

func TestParseNonZeroInt(t *testing.T) {
	if parseNonZeroInt("50", 10) != 50 || parseNonZeroInt("abc", 10) != 10 || parseNonZeroInt("0", 10) != 10 {
		t.Fatal("parseNonZeroInt")
	}
}

func TestTmeChannelLinkToChatID(t *testing.T) {
	a := &app{}
	chatID, msgID, err := a.resolveTmePost(nil, "https://t.me/c/1550117445/42")
	if err != nil {
		t.Fatal(err)
	}
	if msgID != 42 || chatID != -1001550117445 {
		t.Fatalf("got chat=%d msg=%d", chatID, msgID)
	}
	if _, _, err := a.resolveTmePost(nil, "t.me/c/abc/1"); err == nil {
		t.Fatal("bad channel id should error")
	}
	if _, _, err := a.resolveTmePost(nil, "https://example.com/x"); err == nil {
		t.Fatal("unsupported link should error")
	}
}
