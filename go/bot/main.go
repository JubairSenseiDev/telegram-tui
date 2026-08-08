package main

import (
	"fmt"
	"log"
	"os"
	"strings"

	"github.com/PaulSonOfLars/gotgbot/v2"
	"github.com/PaulSonOfLars/gotgbot/v2/ext"
	"github.com/PaulSonOfLars/gotgbot/v2/ext/handlers"
)

type config struct {
	AdminIDs   map[int64]bool
	SourceChat int64
	TargetChat int64
	DataDir    string
}

func env(key, def string) string {
	if v := strings.TrimSpace(os.Getenv(key)); v != "" {
		return v
	}
	return def
}

func parseIDs(s string) map[int64]bool {
	ids := map[int64]bool{}
	for _, p := range strings.Split(s, ",") {
		var id int64
		if _, err := fmt.Sscanf(strings.TrimSpace(p), "%d", &id); err == nil && id != 0 {
			ids[id] = true
		}
	}
	return ids
}

func parseChatID(s string) int64 {
	var id int64
	if s == "" {
		return 0
	}
	if _, err := fmt.Sscanf(strings.TrimSpace(s), "%d", &id); err != nil {
		return 0
	}
	return id
}

func main() {
	token := strings.TrimSpace(os.Getenv("TELEGRAM_BOT_TOKEN"))
	if token == "" {
		log.Fatal("TELEGRAM_BOT_TOKEN is required")
	}

	cfg := config{
		AdminIDs:   parseIDs(os.Getenv("ADMIN_USER_IDS")),
		SourceChat: parseChatID(os.Getenv("SOURCE_CHAT_ID")),
		TargetChat: parseChatID(os.Getenv("TARGET_CHAT_ID")),
		DataDir:    env("DATA_DIR", "data"),
	}

	app, err := newApp(cfg)
	if err != nil {
		log.Fatalf("init: %v", err)
	}

	bot, err := gotgbot.NewBot(token, nil)
	if err != nil {
		log.Fatalf("create bot: %v", err)
	}
	app.bot = bot

	dispatcher := ext.NewDispatcher(&ext.DispatcherOpts{
		Error: func(_ *gotgbot.Bot, _ *ext.Context, err error) ext.DispatcherAction {
			log.Printf("handler error: %v", err)
			return ext.DispatcherActionNoop
		},
		MaxRoutines: ext.DefaultMaxRoutines,
	})
	updater := ext.NewUpdater(dispatcher, nil)

	// command handlers
	dispatcher.AddHandler(handlers.NewCommand("start", app.cmdStart))
	dispatcher.AddHandler(handlers.NewCommand("help", app.cmdHelp))
	dispatcher.AddHandler(handlers.NewCommand("ping", app.cmdPing))
	dispatcher.AddHandler(handlers.NewCommand("subcount", app.cmdSubCount))
	dispatcher.AddHandler(handlers.NewCommand("broadcast", app.cmdBroadcast))
	dispatcher.AddHandler(handlers.NewCommand("list", app.cmdList))
	dispatcher.AddHandler(handlers.NewCommand("addkeyword", app.cmdAddKeyword))
	dispatcher.AddHandler(handlers.NewCommand("delkeyword", app.cmdDelKeyword))
	dispatcher.AddHandler(handlers.NewCommand("keywords", app.cmdKeywords))
	dispatcher.AddHandler(handlers.NewCommand("schedule", app.cmdSchedule))
	dispatcher.AddHandler(handlers.NewCommand("schedules", app.cmdSchedules))
	dispatcher.AddHandler(handlers.NewCommand("scheduledel", app.cmdScheduleDel))
	// everything else: keyword replies + mirror forwarding
	dispatcher.AddHandler(handlers.NewMessage(nil, app.onMessage))

	go app.runScheduler()

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
