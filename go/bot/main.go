package main

import (
	"fmt"
	"log"
	"os"
	"strings"
	"time"

	"github.com/PaulSonOfLars/gotgbot/v2"
	"github.com/PaulSonOfLars/gotgbot/v2/ext"
	"github.com/PaulSonOfLars/gotgbot/v2/ext/handlers"
)

func main() {
	token := strings.TrimSpace(os.Getenv("TELEGRAM_BOT_TOKEN"))
	if token == "" {
		log.Fatal("TELEGRAM_BOT_TOKEN is required")
	}

	bot, err := gotgbot.NewBot(token, nil)
	if err != nil {
		log.Fatalf("create bot: %v", err)
	}

	dispatcher := ext.NewDispatcher(&ext.DispatcherOpts{
		Error: func(_ *gotgbot.Bot, _ *ext.Context, err error) ext.DispatcherAction {
			log.Printf("handler error: %v", err)
			return ext.DispatcherActionNoop
		},
		MaxRoutines: ext.DefaultMaxRoutines,
	})
	updater := ext.NewUpdater(dispatcher, nil)

	dispatcher.AddHandler(handlers.NewCommand("start", start))
	dispatcher.AddHandler(handlers.NewCommand("help", help))
	dispatcher.AddHandler(handlers.NewCommand("ping", ping))
	dispatcher.AddHandler(handlers.NewMessage(nil, echo))

	log.Printf("starting @%s", bot.User.Username)
	if err := updater.StartPolling(bot, &ext.PollingOpts{
		DropPendingUpdates: true,
		GetUpdatesOpts: &gotgbot.GetUpdatesOpts{
			Timeout: 30,
		},
	}); err != nil {
		log.Fatalf("start polling: %v", err)
	}

	updater.Idle()
}

func start(bot *gotgbot.Bot, ctx *ext.Context) error {
	return reply(bot, ctx, "telegram-tui Go bot is online. Use /help for commands.")
}

func help(bot *gotgbot.Bot, ctx *ext.Context) error {
	return reply(bot, ctx, strings.Join([]string{
		"telegram-tui Go bot",
		"/start - bot status",
		"/ping - latency check",
		"/help - commands",
		"Any plain message is echoed back.",
	}, "\n"))
}

func ping(bot *gotgbot.Bot, ctx *ext.Context) error {
	return reply(bot, ctx, fmt.Sprintf("pong %s", time.Now().Format(time.RFC3339)))
}

func echo(bot *gotgbot.Bot, ctx *ext.Context) error {
	if ctx.EffectiveMessage == nil || strings.HasPrefix(ctx.EffectiveMessage.Text, "/") {
		return nil
	}
	return reply(bot, ctx, ctx.EffectiveMessage.Text)
}

func reply(bot *gotgbot.Bot, ctx *ext.Context, text string) error {
	if ctx.EffectiveChat == nil {
		return nil
	}
	_, err := bot.SendMessage(ctx.EffectiveChat.Id, text, nil)
	return err
}
