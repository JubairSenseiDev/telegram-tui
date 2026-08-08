package main

import (
	"fmt"
	"sort"
	"strings"
	"time"

	"github.com/PaulSonOfLars/gotgbot/v2"
	"github.com/PaulSonOfLars/gotgbot/v2/ext"
)

const helpText = `telegram-tui bot

General:
/start - subscribe to broadcasts
/help - this message
/ping - latency check
/subcount - number of subscribers

Admin:
/broadcast <text> - send to all subscribers
/list - list subscribers
/ids - list all known user & chat IDs
/addkeyword <word>|<reply> - add keyword auto-reply
/delkeyword <word> - remove keyword auto-reply
/keywords - list keyword auto-replies
/schedule <seconds> <text> - repeat message every N seconds
/schedules - list schedules
/scheduledel <id> - delete a schedule

Assistant (grab & share posts, no download):
/getpost <link> [target] - copy a post from a t.me/... link into a chat
/afwd <id> [target] - forward a saved post to a chat
/addchannel <id|@user|t.me link> - watch a channel/group, archive every post
/removechannel <id> - stop watching a channel
/listchannels - list watched channels`

// replyMsg is a helper wrapping Message.Reply, discarding the sent Message.
func replyMsg(msg *gotgbot.Message, b *gotgbot.Bot, text string, opts *gotgbot.SendMessageOpts) error {
	_, err := msg.Reply(b, text, opts)
	return err
}

func (a *app) cmdStart(b *gotgbot.Bot, ctx *ext.Context) error {
	msg := ctx.EffectiveMessage
	if msg == nil || msg.From == nil {
		return nil
	}
	new := a.subscribe(msg.From.Id, msg.Chat.Id, msg.From.FirstName, msg.From.Username)
	text := "You are now subscribed to broadcasts."
	if new {
		text = "Welcome! You are now subscribed to broadcasts. Use /help for commands."
	}
	return replyMsg(msg, b, text, nil)
}

func (a *app) cmdHelp(b *gotgbot.Bot, ctx *ext.Context) error {
	if ctx.EffectiveMessage == nil {
		return nil
	}
	return replyMsg(ctx.EffectiveMessage, b, helpText, nil)
}

func (a *app) cmdPing(b *gotgbot.Bot, ctx *ext.Context) error {
	if ctx.EffectiveMessage == nil {
		return nil
	}
	return replyMsg(ctx.EffectiveMessage, b, fmt.Sprintf("pong %s", time.Now().Format(time.RFC3339)), nil)
}

func (a *app) cmdSubCount(b *gotgbot.Bot, ctx *ext.Context) error {
	if ctx.EffectiveMessage == nil {
		return nil
	}
	n := len(a.sortedSubscribers())
	return replyMsg(ctx.EffectiveMessage, b, fmt.Sprintf("subscribers: %d", n), nil)
}

func (a *app) cmdBroadcast(b *gotgbot.Bot, ctx *ext.Context) error {
	msg := ctx.EffectiveMessage
	if msg == nil {
		return nil
	}
	if !a.isAdmin(msg.From.Id) {
		return replyMsg(msg, b, "Only admins can broadcast.", nil)
	}
	text := strings.TrimSpace(strings.TrimPrefix(msg.Text, "/broadcast"))
	if text == "" {
		return replyMsg(msg, b, "Usage: /broadcast <text>", nil)
	}
	subs := a.sortedSubscribers()
	if len(subs) == 0 {
		return replyMsg(msg, b, "No subscribers yet.", nil)
	}
	sent, failed := 0, 0
	for _, s := range subs {
		if _, err := b.SendMessage(s.ChatID, text, nil); err != nil {
			failed++
			continue
		}
		sent++
		time.Sleep(1 * time.Second)
	}
	return replyMsg(msg, b, fmt.Sprintf("broadcast done: %d sent, %d failed (of %d)", sent, failed, len(subs)), nil)
}

func (a *app) cmdList(b *gotgbot.Bot, ctx *ext.Context) error {
	msg := ctx.EffectiveMessage
	if msg == nil {
		return nil
	}
	if !a.isAdmin(msg.From.Id) {
		return replyMsg(msg, b, "Only admins can list subscribers.", nil)
	}
	subs := a.sortedSubscribers()
	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("subscribers (%d):\n", len(subs)))
	for _, s := range subs {
		name := s.Name
		if name == "" {
			name = "?"
		}
		fmt.Fprintf(&sb, "• %s (@%s) id=%d\n", name, s.Username, s.ID)
	}
	return replyMsg(msg, b, strings.TrimSuffix(sb.String(), "\n"), nil)
}

func (a *app) cmdIds(b *gotgbot.Bot, ctx *ext.Context) error {
	msg := ctx.EffectiveMessage
	if msg == nil {
		return nil
	}
	if !a.isAdmin(msg.From.Id) {
		return replyMsg(msg, b, "Only admins can list IDs.", nil)
	}
	seen := a.sortedSeen()
	if len(seen) == 0 {
		return replyMsg(msg, b, "no users seen yet.", nil)
	}
	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("known users (%d):\n", len(seen)))
	for _, sc := range seen {
		name := sc.Name
		if name == "" {
			name = "?"
		}
		uname := sc.Username
		if uname == "" {
			uname = "-"
		}
		fmt.Fprintf(&sb, "id=%d | chat=%d | @%s | %s\n", sc.ID, sc.ChatID, uname, name)
	}
	return replyMsg(msg, b, strings.TrimSuffix(sb.String(), "\n"), nil)
}

func (a *app) cmdAddKeyword(b *gotgbot.Bot, ctx *ext.Context) error {
	msg := ctx.EffectiveMessage
	if msg == nil {
		return nil
	}
	if !a.isAdmin(msg.From.Id) {
		return replyMsg(msg, b, "Only admins can add keywords.", nil)
	}
	arg := strings.TrimSpace(strings.TrimPrefix(msg.Text, "/addkeyword"))
	kw, replyText, ok := strings.Cut(arg, "|")
	kw, replyText = strings.TrimSpace(kw), strings.TrimSpace(replyText)
	if !ok || kw == "" || replyText == "" {
		return replyMsg(msg, b, "Usage: /addkeyword <word>|<reply>", nil)
	}
	a.mu.Lock()
	a.keywords[strings.ToLower(kw)] = replyText
	a.mu.Unlock()
	a.saveAll()
	return replyMsg(msg, b, fmt.Sprintf("keyword added: %q", kw), nil)
}

func (a *app) cmdDelKeyword(b *gotgbot.Bot, ctx *ext.Context) error {
	msg := ctx.EffectiveMessage
	if msg == nil {
		return nil
	}
	if !a.isAdmin(msg.From.Id) {
		return replyMsg(msg, b, "Only admins can delete keywords.", nil)
	}
	kw := strings.ToLower(strings.TrimSpace(strings.TrimPrefix(msg.Text, "/delkeyword")))
	if kw == "" {
		return replyMsg(msg, b, "Usage: /delkeyword <word>", nil)
	}
	a.mu.Lock()
	_, ok := a.keywords[kw]
	delete(a.keywords, kw)
	a.mu.Unlock()
	if !ok {
		return replyMsg(msg, b, fmt.Sprintf("no such keyword: %q", kw), nil)
	}
	a.saveAll()
	return replyMsg(msg, b, fmt.Sprintf("keyword deleted: %q", kw), nil)
}

func (a *app) cmdKeywords(b *gotgbot.Bot, ctx *ext.Context) error {
	msg := ctx.EffectiveMessage
	if msg == nil {
		return nil
	}
	if !a.isAdmin(msg.From.Id) {
		return replyMsg(msg, b, "Only admins can list keywords.", nil)
	}
	a.mu.Lock()
	keys := make([]string, 0, len(a.keywords))
	for k := range a.keywords {
		keys = append(keys, k)
	}
	kws := map[string]string{}
	for k, v := range a.keywords {
		kws[k] = v
	}
	a.mu.Unlock()
	sort.Strings(keys)
	if len(keys) == 0 {
		return replyMsg(msg, b, "no keywords configured.", nil)
	}
	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("keywords (%d):\n", len(keys)))
	for _, k := range keys {
		fmt.Fprintf(&sb, "• %q → %s\n", k, kws[k])
	}
	return replyMsg(msg, b, strings.TrimSuffix(sb.String(), "\n"), nil)
}

func (a *app) cmdSchedule(b *gotgbot.Bot, ctx *ext.Context) error {
	msg := ctx.EffectiveMessage
	if msg == nil {
		return nil
	}
	if !a.isAdmin(msg.From.Id) {
		return replyMsg(msg, b, "Only admins can add schedules.", nil)
	}
	arg := strings.TrimSpace(strings.TrimPrefix(msg.Text, "/schedule"))
	fields := strings.SplitN(arg, " ", 2)
	if len(fields) < 2 {
		return replyMsg(msg, b, "Usage: /schedule <seconds> <text>", nil)
	}
	var interval int64
	if _, err := fmt.Sscanf(fields[0], "%d", &interval); err != nil || interval < 10 {
		return replyMsg(msg, b, "interval must be a number of seconds (min 10).", nil)
	}
	text := strings.TrimSpace(fields[1])
	if text == "" {
		return replyMsg(msg, b, "message text missing.", nil)
	}
	a.mu.Lock()
	item := scheduleItem{
		ID:       a.nextID,
		ChatID:   msg.Chat.Id,
		Text:     text,
		Interval: interval,
		Last:     time.Now().Unix(),
	}
	a.nextID++
	a.schedules = append(a.schedules, item)
	a.mu.Unlock()
	a.saveAll()
	return replyMsg(msg, b, fmt.Sprintf("schedule #%d added (every %ds).", item.ID, interval), nil)
}

func (a *app) cmdSchedules(b *gotgbot.Bot, ctx *ext.Context) error {
	msg := ctx.EffectiveMessage
	if msg == nil {
		return nil
	}
	if !a.isAdmin(msg.From.Id) {
		return replyMsg(msg, b, "Only admins can list schedules.", nil)
	}
	a.mu.Lock()
	items := append([]scheduleItem(nil), a.schedules...)
	a.mu.Unlock()
	if len(items) == 0 {
		return replyMsg(msg, b, "no schedules.", nil)
	}
	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("schedules (%d):\n", len(items)))
	for _, s := range items {
		fmt.Fprintf(&sb, "#%d every %ds in chat %d: %s\n", s.ID, s.Interval, s.ChatID, s.Text)
	}
	return replyMsg(msg, b, strings.TrimSuffix(sb.String(), "\n"), nil)
}

func (a *app) cmdScheduleDel(b *gotgbot.Bot, ctx *ext.Context) error {
	msg := ctx.EffectiveMessage
	if msg == nil {
		return nil
	}
	if !a.isAdmin(msg.From.Id) {
		return replyMsg(msg, b, "Only admins can delete schedules.", nil)
	}
	var id int64
	if _, err := fmt.Sscanf(strings.TrimPrefix(msg.Text, "/scheduledel"), "%d", &id); err != nil {
		return replyMsg(msg, b, "Usage: /scheduledel <id>", nil)
	}
	a.mu.Lock()
	kept := a.schedules[:0]
	removed := false
	for _, s := range a.schedules {
		if s.ID == id {
			removed = true
			continue
		}
		kept = append(kept, s)
	}
	a.schedules = kept
	a.mu.Unlock()
	if !removed {
		return replyMsg(msg, b, fmt.Sprintf("no schedule #%d.", id), nil)
	}
	a.saveAll()
	return replyMsg(msg, b, fmt.Sprintf("schedule #%d deleted.", id), nil)
}
