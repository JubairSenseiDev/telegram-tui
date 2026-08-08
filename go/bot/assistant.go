package main

import (
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/PaulSonOfLars/gotgbot/v2"
	"github.com/PaulSonOfLars/gotgbot/v2/ext"
)

// watchedChannel is a channel/group the assistant watches and archives.
type watchedChannel struct {
	ID      int64  `json:"id"`
	Title   string `json:"title"`
	Type    string `json:"type"`
	AddedAt int64  `json:"added_at"`
}

// savedPost is one fully-saved post. Text/captions are always stored (tiny);
// media bytes are only downloaded when SAVE_MEDIA is on or via /aget.
type savedPost struct {
	ID         int64  `json:"id"`
	ChatID     int64  `json:"chat_id"`
	ChatTitle  string `json:"chat_title"`
	MessageID  int64  `json:"message_id"`
	Date       int64  `json:"date"`
	EditDate   int64  `json:"edit_date,omitempty"`
	Text       string `json:"text,omitempty"`
	Caption    string `json:"caption,omitempty"`
	MediaType  string `json:"media_type,omitempty"`
	MediaExt   string `json:"media_ext,omitempty"`
	FileID     string `json:"file_id,omitempty"`
	FileSize   int64  `json:"file_size,omitempty"`
	MediaSaved bool   `json:"media_saved,omitempty"`
	MediaPath  string `json:"media_path,omitempty"`
	SourceLink string `json:"source_link,omitempty"`
	Forwarded  bool   `json:"forwarded,omitempty"`
}

func (p *savedPost) body() string {
	if p.Caption != "" {
		return p.Text + "\n\n" + p.Caption
	}
	return p.Text
}

func (a *app) postPath() string {
	return a.path("assistant_posts.jsonl")
}

func (a *app) mediaDir() string {
	dir := filepath.Join(a.cfg.DataDir, "assistant", "media")
	_ = os.MkdirAll(dir, 0o700)
	return dir
}

func (a *app) isWatched(chatID int64) bool {
	a.mu.Lock()
	defer a.mu.Unlock()
	_, ok := a.watched[chatID]
	return ok
}

func (a *app) sortedWatched() []watchedChannel {
	a.mu.Lock()
	defer a.mu.Unlock()
	out := make([]watchedChannel, 0, len(a.watched))
	for _, w := range a.watched {
		out = append(out, w)
	}
	sort.Slice(out, func(i, j int) bool { return out[i].AddedAt < out[j].AddedAt })
	return out
}

// assistantOnMessage is called for every message. It saves posts from watched
// channels/groups and anything forwarded to the bot directly.
func (a *app) assistantOnMessage(b *gotgbot.Bot, msg *gotgbot.Message) {
	if msg == nil {
		return
	}
	if a.isWatched(msg.Chat.Id) {
		a.savePostFromMessage(b, msg, false)
		return
	}
	if msg.Chat.Type == "private" && msg.ForwardOrigin != nil {
		a.savePostFromMessage(b, msg, true)
	}
}

func (a *app) savePostFromMessage(b *gotgbot.Bot, msg *gotgbot.Message, forwarded bool) {
	p := &savedPost{
		ChatID:    msg.Chat.Id,
		ChatTitle: msg.Chat.Title,
		MessageID: msg.MessageId,
		Date:      msg.Date,
		EditDate:  msg.EditDate,
		Text:      msg.Text,
		Caption:   msg.Caption,
	}
	if p.ChatTitle == "" {
		p.ChatTitle = msg.Chat.Username
	}
	mt, fid, size, ext := mediaOf(msg)
	p.MediaType, p.MediaExt, p.FileID, p.FileSize = mt, ext, fid, size
	if p.Text == "" && p.Caption == "" && p.MediaType == "" {
		return
	}

	var key string
	if forwarded && msg.ForwardOrigin != nil {
		mo := msg.ForwardOrigin.MergeMessageOrigin()
		var origChatID int64
		if mo.Chat != nil {
			origChatID = mo.Chat.Id
			if p.ChatTitle == "" {
				p.ChatTitle = mo.Chat.Title
			}
		} else if mo.SenderChat != nil {
			origChatID = mo.SenderChat.Id
			if p.ChatTitle == "" {
				p.ChatTitle = mo.SenderChat.Title
			}
		}
		p.Forwarded = true
		if mo.MessageId != 0 {
			key = fmt.Sprintf("f:%d:%d", origChatID, mo.MessageId)
			p.SourceLink = tmeLink(origChatID, mo.MessageId)
		} else {
			key = fmt.Sprintf("f:%d:%d", origChatID, mo.Date)
		}
	} else {
		key = fmt.Sprintf("%d:%d", msg.Chat.Id, msg.MessageId)
		p.SourceLink = tmeLink(msg.Chat.Id, msg.MessageId)
	}

	a.savePost(b, p, key)
}

// savePost stores a post (or updates it on edit) and persists it locally.
func (a *app) savePost(b *gotgbot.Bot, p *savedPost, key string) {
	a.mu.Lock()
	if id, ok := a.postIdx[key]; ok {
		for i := range a.posts {
			if a.posts[i].ID == id {
				a.posts[i].EditDate = p.EditDate
				a.posts[i].Text = p.Text
				a.posts[i].Caption = p.Caption
				a.posts[i].MediaType = p.MediaType
				a.posts[i].FileID = p.FileID
				a.posts[i].FileSize = p.FileSize
			}
		}
		a.mu.Unlock()
		a.rewritePosts()
		return
	}
	p.ID = a.assistNext
	a.assistNext++
	a.posts = append(a.posts, *p)
	a.postIdx[key] = p.ID
	a.mu.Unlock()
	a.appendPostLine(p)
	a.maybeSaveMedia(b, p, false)
}

func (a *app) maybeSaveMedia(b *gotgbot.Bot, p *savedPost, force bool) {
	if !force && !a.cfg.SaveMedia {
		return
	}
	if p.FileID == "" || p.MediaSaved {
		return
	}
	if !force && p.FileSize > int64(a.cfg.MaxMediaMB)*1024*1024 {
		log.Printf("assistant: media #%d too big (%.1f MB), skipping", p.ID, float64(p.FileSize)/1048576)
		return
	}
	ext := p.MediaExt
	if ext == "" {
		ext = "bin"
	}
	path := filepath.Join(a.mediaDir(), fmt.Sprintf("%d.%s", p.ID, ext))
	if err := a.downloadFile(b, p.FileID, path); err != nil {
		log.Printf("assistant: download #%d: %v", p.ID, err)
		return
	}
	a.mu.Lock()
	for i := range a.posts {
		if a.posts[i].ID == p.ID {
			a.posts[i].MediaSaved = true
			a.posts[i].MediaPath = path
		}
	}
	a.mu.Unlock()
	a.rewritePosts()
}

func (a *app) downloadFile(b *gotgbot.Bot, fileID, dest string) error {
	f, err := b.GetFile(fileID, nil)
	if err != nil {
		return err
	}
	url := fmt.Sprintf("https://api.telegram.org/file/bot%s/%s", a.cfg.Token, f.FilePath)
	resp, err := http.Get(url)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("download %s: %s", url, resp.Status)
	}
	out, err := os.Create(dest)
	if err != nil {
		return err
	}
	defer out.Close()
	_, err = io.Copy(out, resp.Body)
	return err
}

func (a *app) appendPostLine(p *savedPost) {
	line, err := json.Marshal(p)
	if err != nil {
		return
	}
	f, err := os.OpenFile(a.postPath(), os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0o600)
	if err != nil {
		log.Printf("assistant: append post: %v", err)
		return
	}
	defer f.Close()
	_, _ = f.Write(append(line, '\n'))
}

func (a *app) rewritePosts() {
	a.mu.Lock()
	posts := append([]savedPost(nil), a.posts...)
	a.mu.Unlock()
	var sb strings.Builder
	enc := json.NewEncoder(&sb)
	for i := range posts {
		_ = enc.Encode(&posts[i])
	}
	if err := os.WriteFile(a.postPath(), []byte(sb.String()), 0o600); err != nil {
		log.Printf("assistant: rewrite posts: %v", err)
	}
}

func loadPosts(path string) ([]savedPost, map[string]int64, int64) {
	posts := []savedPost{}
	idx := map[string]int64{}
	next := int64(1)
	data, err := os.ReadFile(path)
	if err != nil {
		return posts, idx, next
	}
	for _, line := range strings.Split(string(data), "\n") {
		if strings.TrimSpace(line) == "" {
			continue
		}
		var p savedPost
		if err := json.Unmarshal([]byte(line), &p); err != nil {
			log.Printf("assistant: corrupt post line: %v", err)
			continue
		}
		posts = append(posts, p)
		if p.ID >= next {
			next = p.ID + 1
		}
	}
	// rebuild dedupe index
	for _, p := range posts {
		if p.MessageID != 0 {
			idx[fmt.Sprintf("%d:%d", p.ChatID, p.MessageID)] = p.ID
		}
	}
	return posts, idx, next
}

// mediaOf extracts (type, file_id, size, extension) from a message.
func mediaOf(msg *gotgbot.Message) (string, string, int64, string) {
	if v := msg.Video; v != nil {
		return "video", v.FileId, v.FileSize, "mp4"
	}
	if len(msg.Photo) > 0 {
		p := msg.Photo[len(msg.Photo)-1]
		return "photo", p.FileId, p.FileSize, "jpg"
	}
	if d := msg.Document; d != nil {
		ext := strings.TrimPrefix(filepath.Ext(d.FileName), ".")
		if ext == "" {
			ext = "bin"
		}
		return "document", d.FileId, d.FileSize, ext
	}
	if a := msg.Audio; a != nil {
		return "audio", a.FileId, a.FileSize, "m4a"
	}
	if v := msg.Voice; v != nil {
		return "voice", v.FileId, v.FileSize, "ogg"
	}
	if v := msg.VideoNote; v != nil {
		return "video_note", v.FileId, v.FileSize, "mp4"
	}
	if an := msg.Animation; an != nil {
		return "animation", an.FileId, an.FileSize, "gif"
	}
	if s := msg.Sticker; s != nil {
		return "sticker", s.FileId, s.FileSize, "webp"
	}
	return "", "", 0, ""
}

// tmeLink builds a t.me/... link for a post.
func tmeLink(chatID, msgID int64) string {
	cid := chatID
	if cid < 0 {
		s := strconv.FormatInt(-cid, 10)
		s = strings.TrimPrefix(s, "100")
		if n, err := strconv.ParseInt(s, 10, 64); err == nil {
			cid = n
		}
	}
	return fmt.Sprintf("https://t.me/c/%d/%d", cid, msgID)
}

// resolveChat turns a numeric id, @username or t.me link into a chat id + info.
func (a *app) resolveChat(b *gotgbot.Bot, arg string) (int64, gotgbot.ChatFullInfo, error) {
	arg = strings.TrimSpace(arg)
	if n, err := strconv.ParseInt(arg, 10, 64); err == nil {
		info, err := b.GetChat(n, nil)
		if err != nil {
			return 0, gotgbot.ChatFullInfo{}, err
		}
		return n, *info, nil
	}
	u := strings.TrimPrefix(arg, "@")
	for _, pre := range []string{"https://t.me/", "http://t.me/", "https://telegram.me/", "http://telegram.me/", "t.me/", "telegram.me/"} {
		u = strings.TrimPrefix(u, pre)
	}
	u = strings.TrimSuffix(strings.SplitN(u, "/", 2)[0], "/")
	if u == "" {
		return 0, gotgbot.ChatFullInfo{}, fmt.Errorf("empty username")
	}
	raw, err := b.Request("getChat", map[string]any{"chat_id": "@" + u}, nil)
	if err != nil {
		return 0, gotgbot.ChatFullInfo{}, err
	}
	var info gotgbot.ChatFullInfo
	if err := json.Unmarshal(raw, &info); err != nil {
		return 0, gotgbot.ChatFullInfo{}, err
	}
	return info.Id, info, nil
}

func (a *app) cmdAddChannel(b *gotgbot.Bot, ctx *ext.Context) error {
	msg := ctx.EffectiveMessage
	if msg == nil {
		return nil
	}
	if !a.isAdmin(msg.From.Id) {
		return replyMsg(msg, b, "Only admins can add channels.", nil)
	}
	arg := strings.TrimSpace(strings.TrimPrefix(msg.Text, "/addchannel"))
	if arg == "" {
		return replyMsg(msg, b, "Usage: /addchannel <chat id> | <@username> | <t.me/...>", nil)
	}
	chatID, info, err := a.resolveChat(b, arg)
	if err != nil {
		return replyMsg(msg, b, fmt.Sprintf("could not resolve %q: %v", arg, err), nil)
	}
	if info.Type != "channel" && info.Type != "supergroup" && info.Type != "group" {
		return replyMsg(msg, b, fmt.Sprintf("%q is a %s — only channels/groups can be watched.", arg, info.Type), nil)
	}
	a.mu.Lock()
	if _, ok := a.watched[chatID]; ok {
		a.mu.Unlock()
		return replyMsg(msg, b, fmt.Sprintf("%s is already being watched.", info.Title), nil)
	}
	a.watched[chatID] = watchedChannel{ID: chatID, Title: info.Title, Type: info.Type, AddedAt: time.Now().Unix()}
	a.mu.Unlock()
	a.saveAll()
	note := ""
	if info.Type == "channel" {
		note = " (add the bot as an admin of the channel to receive its posts)"
	}
	return replyMsg(msg, b, fmt.Sprintf("Now watching %s (id %d).%s", info.Title, chatID, note), nil)
}

func (a *app) cmdRemoveChannel(b *gotgbot.Bot, ctx *ext.Context) error {
	msg := ctx.EffectiveMessage
	if msg == nil {
		return nil
	}
	if !a.isAdmin(msg.From.Id) {
		return replyMsg(msg, b, "Only admins can remove channels.", nil)
	}
	var id int64
	if _, err := fmt.Sscanf(strings.TrimPrefix(msg.Text, "/removechannel"), "%d", &id); err != nil {
		return replyMsg(msg, b, "Usage: /removechannel <chat id>", nil)
	}
	a.mu.Lock()
	title := ""
	if w, ok := a.watched[id]; ok {
		title = w.Title
		delete(a.watched, id)
	}
	a.mu.Unlock()
	if title == "" {
		return replyMsg(msg, b, fmt.Sprintf("not watching %d.", id), nil)
	}
	a.saveAll()
	return replyMsg(msg, b, fmt.Sprintf("Stopped watching %s (%d).", title, id), nil)
}

func (a *app) cmdListChannels(b *gotgbot.Bot, ctx *ext.Context) error {
	msg := ctx.EffectiveMessage
	if msg == nil {
		return nil
	}
	if !a.isAdmin(msg.From.Id) {
		return replyMsg(msg, b, "Only admins can list channels.", nil)
	}
	watched := a.sortedWatched()
	if len(watched) == 0 {
		return replyMsg(msg, b, "no channels being watched. Use /addchannel.", nil)
	}
	var sb strings.Builder
	fmt.Fprintf(&sb, "watching (%d):\n", len(watched))
	for _, w := range watched {
		fmt.Fprintf(&sb, "• %s (%s) id=%d\n", w.Title, w.Type, w.ID)
	}
	return replyMsg(msg, b, strings.TrimSuffix(sb.String(), "\n"), nil)
}

func (a *app) cmdAStats(b *gotgbot.Bot, ctx *ext.Context) error {
	msg := ctx.EffectiveMessage
	if msg == nil {
		return nil
	}
	if !a.isAdmin(msg.From.Id) {
		return replyMsg(msg, b, "Only admins can see stats.", nil)
	}
	a.mu.Lock()
	n := len(a.posts)
	watched := len(a.watched)
	var mediaBytes, savedBytes int64
	mediaPosts := 0
	savedMedia := 0
	for _, p := range a.posts {
		if p.FileID != "" {
			mediaPosts++
			mediaBytes += p.FileSize
			if p.MediaSaved {
				savedMedia++
				if st, err := os.Stat(p.MediaPath); err == nil {
					savedBytes += st.Size()
				}
			}
		}
	}
	a.mu.Unlock()
	text := fmt.Sprintf(
		"assistant stats:\nsaved posts: %d\nwatched channels: %d\nposts with media: %d\nmedia metadata: %s\nmedia actually downloaded: %d (%s)",
		n, watched, mediaPosts, humanSize(mediaBytes), savedMedia, humanSize(savedBytes),
	)
	return replyMsg(msg, b, text, nil)
}

func (a *app) cmdASearch(b *gotgbot.Bot, ctx *ext.Context) error {
	msg := ctx.EffectiveMessage
	if msg == nil {
		return nil
	}
	if !a.isAdmin(msg.From.Id) {
		return replyMsg(msg, b, "Only admins can search.", nil)
	}
	q := strings.ToLower(strings.TrimSpace(strings.TrimPrefix(msg.Text, "/asearch")))
	if q == "" {
		return replyMsg(msg, b, "Usage: /asearch <query>", nil)
	}
	a.mu.Lock()
	var hits []savedPost
	for _, p := range a.posts {
		if strings.Contains(strings.ToLower(p.Text+"\n"+p.Caption), q) {
			hits = append(hits, p)
		}
		if len(hits) >= 10 {
			break
		}
	}
	a.mu.Unlock()
	if len(hits) == 0 {
		return replyMsg(msg, b, "no saved posts match.", nil)
	}
	var sb strings.Builder
	fmt.Fprintf(&sb, "matches (%d):\n", len(hits))
	for _, p := range hits {
		snippet := strings.ReplaceAll(p.body(), "\n", " ")
		if len(snippet) > 90 {
			snippet = snippet[:90] + "…"
		}
		fmt.Fprintf(&sb, "#%d [%s] %s: %s\n", p.ID, p.ChatTitle, time.Unix(p.Date, 0).Format("2006-01-02"), snippet)
	}
	return replyMsg(msg, b, strings.TrimSuffix(sb.String(), "\n"), nil)
}

func (a *app) cmdASave(b *gotgbot.Bot, ctx *ext.Context) error {
	msg := ctx.EffectiveMessage
	if msg == nil {
		return nil
	}
	if !a.isAdmin(msg.From.Id) {
		return replyMsg(msg, b, "Only admins can save posts.", nil)
	}
	repl := msg.ReplyToMessage
	if repl == nil {
		return replyMsg(msg, b, "Reply to a message to save it, e.g. /asave", nil)
	}
	a.savePostFromMessage(b, repl, false)
	return replyMsg(msg, b, "post saved locally. See /astats.", nil)
}

func (a *app) cmdAGet(b *gotgbot.Bot, ctx *ext.Context) error {
	msg := ctx.EffectiveMessage
	if msg == nil {
		return nil
	}
	if !a.isAdmin(msg.From.Id) {
		return replyMsg(msg, b, "Only admins can fetch media.", nil)
	}
	var id int64
	if _, err := fmt.Sscanf(strings.TrimPrefix(msg.Text, "/aget"), "%d", &id); err != nil {
		return replyMsg(msg, b, "Usage: /aget <post id>", nil)
	}
	a.mu.Lock()
	var p *savedPost
	for i := range a.posts {
		if a.posts[i].ID == id {
			p = &a.posts[i]
			break
		}
	}
	a.mu.Unlock()
	if p == nil {
		return replyMsg(msg, b, fmt.Sprintf("no post #%d.", id), nil)
	}
	if p.FileID == "" {
		return replyMsg(msg, b, fmt.Sprintf("post #%d has no media.\n\n%s", id, p.body()), nil)
	}
	if p.MediaSaved {
		return replyMsg(msg, b, fmt.Sprintf("post #%d media already saved:\n%s", id, p.MediaPath), nil)
	}
	if p.FileSize > int64(a.cfg.MaxMediaMB)*1024*1024 {
		return replyMsg(msg, b, fmt.Sprintf("post #%d media is %s — larger than the %d MB cap. Raise ASSISTANT_MAX_MEDIA_MB to allow it.", id, humanSize(p.FileSize), a.cfg.MaxMediaMB), nil)
	}
	replyMsg(msg, b, fmt.Sprintf("Downloading media for post #%d (%s)…", id, humanSize(p.FileSize)), nil)
	a.maybeSaveMedia(b, p, true)
	a.mu.Lock()
	for i := range a.posts {
		if a.posts[i].ID == id {
			p = &a.posts[i]
			break
		}
	}
	a.mu.Unlock()
	if !p.MediaSaved {
		return replyMsg(msg, b, fmt.Sprintf("download failed for post #%d.", id), nil)
	}
	return replyMsg(msg, b, fmt.Sprintf("post #%d media saved:\n%s", id, p.MediaPath), nil)
}

func (a *app) cmdAExport(b *gotgbot.Bot, ctx *ext.Context) error {
	msg := ctx.EffectiveMessage
	if msg == nil {
		return nil
	}
	if !a.isAdmin(msg.From.Id) {
		return replyMsg(msg, b, "Only admins can export.", nil)
	}
	a.mu.Lock()
	posts := append([]savedPost(nil), a.posts...)
	a.mu.Unlock()
	if len(posts) == 0 {
		return replyMsg(msg, b, "no saved posts to export.", nil)
	}
	dir := filepath.Join(a.cfg.DataDir, "assistant")
	_ = os.MkdirAll(dir, 0o700)
	path := filepath.Join(dir, fmt.Sprintf("archive-%s.html", time.Now().Format("20060102-150405")))
	if err := writeHTML(posts, path); err != nil {
		return replyMsg(msg, b, fmt.Sprintf("export failed: %v", err), nil)
	}
	return replyMsg(msg, b, fmt.Sprintf("exported %d posts (offline HTML):\n%s", len(posts), path), nil)
}

func writeHTML(posts []savedPost, path string) error {
	var sb strings.Builder
	sb.WriteString("<!doctype html><html><head><meta charset=\"utf-8\">")
	sb.WriteString("<title>telegram-tui assistant archive</title>")
	sb.WriteString("<style>body{font-family:system-ui;max-width:760px;margin:20px auto;padding:0 12px}")
	sb.WriteString("h1{font-size:1.4em}.post{border:1px solid #ddd;border-radius:8px;padding:10px 14px;margin:14px 0}")
	sb.WriteString(".meta{color:#888;font-size:.85em;margin-bottom:6px}.media{color:#2563eb;font-size:.9em}")
	sb.WriteString("pre{white-space:pre-wrap}</style></head><body>")
	fmt.Fprintf(&sb, "<h1>telegram-tui assistant archive</h1><p>%d posts · %s</p>", len(posts), time.Now().Format("2006-01-02 15:04"))
	for _, p := range posts {
		sb.WriteString("<div class=\"post\">")
		date := time.Unix(p.Date, 0).Format("2006-01-02 15:04")
		fmt.Fprintf(&sb, "<div class=\"meta\">#%d · %s · %s · %s</div>", p.ID, date, htmlEsc(p.ChatTitle), htmlEsc(p.SourceLink))
		if p.MediaType != "" {
			size := ""
			if p.FileSize > 0 {
				size = " · " + humanSize(p.FileSize)
			}
			fmt.Fprintf(&sb, "<div class=\"media\">[%s%s]%s</div>", p.MediaType, size, savedMark(p))
		}
		sb.WriteString("<pre>" + htmlEsc(p.body()) + "</pre>")
		sb.WriteString("</div>")
	}
	sb.WriteString("</body></html>")
	return os.WriteFile(path, []byte(sb.String()), 0o600)
}

func savedMark(p savedPost) string {
	if p.FileID != "" && p.MediaSaved {
		return " (downloaded)"
	}
	return ""
}

func htmlEsc(s string) string {
	r := strings.NewReplacer("&", "&amp;", "<", "&lt;", ">", "&gt;", "\"", "&quot;")
	return r.Replace(s)
}

func humanSize(bytes int64) string {
	if bytes < 1024 {
		return fmt.Sprintf("%d B", bytes)
	}
	units := []string{"KB", "MB", "GB", "TB"}
	n := float64(bytes) / 1024
	i := 0
	for n >= 1024 && i < len(units)-1 {
		n /= 1024
		i++
	}
	return fmt.Sprintf("%.1f %s", n, units[i])
}
