package main

import (
	"log"
	"strings"

	"github.com/PaulSonOfLars/gotgbot/v2"
	"github.com/PaulSonOfLars/gotgbot/v2/ext"
)

// onMessage handles everything that is not a command:
// keyword auto-replies, new-member greetings, and channel mirroring.
func (a *app) onMessage(b *gotgbot.Bot, ctx *ext.Context) error {
	msg := ctx.EffectiveMessage
	if msg == nil {
		return nil
	}

	a.greetNewMembers(b, msg)

	// keyword auto-reply
	if msg.Text != "" && !strings.HasPrefix(msg.Text, "/") {
		if reply := a.matchKeyword(msg.Text); reply != "" {
			if _, err := msg.Reply(b, reply, nil); err != nil {
				log.Printf("keyword reply: %v", err)
			}
		}
	}

	// channel / group mirroring
	a.mirror(b, msg)

	return nil
}

func (a *app) greetNewMembers(b *gotgbot.Bot, msg *gotgbot.Message) {
	if len(msg.NewChatMembers) == 0 {
		return
	}
	for _, u := range msg.NewChatMembers {
		if u.Id == b.Id {
			_, _ = msg.Reply(b, "Hello! I am a broadcast bot. Use /help to see what I can do.", nil)
			return
		}
	}
}

func (a *app) matchKeyword(text string) string {
	lower := strings.ToLower(text)
	a.mu.Lock()
	defer a.mu.Unlock()
	for kw, reply := range a.keywords {
		if strings.Contains(lower, kw) {
			return reply
		}
	}
	return ""
}

// mirror forwards messages from the configured source chat to the target chat.
func (a *app) mirror(b *gotgbot.Bot, msg *gotgbot.Message) {
	if a.cfg.SourceChat == 0 || a.cfg.TargetChat == 0 {
		return
	}
	if msg.Chat.Id != a.cfg.SourceChat {
		return
	}
	// never re-forward our own messages (avoids loops)
	if msg.From != nil && msg.From.Id == b.Id {
		return
	}
	if msg.SenderChat != nil && msg.SenderChat.Id == b.Id {
		return
	}
	if _, err := msg.Forward(b, a.cfg.TargetChat, nil); err != nil {
		log.Printf("mirror forward: %v", err)
	}
}
