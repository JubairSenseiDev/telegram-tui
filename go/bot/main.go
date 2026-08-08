package main

import (
	"fmt"
	"log"
	"os"
	"strconv"
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
	Token      string
	SaveMedia  bool
	MaxMediaMB int
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

func hasArg(name string) bool {
	for _, a := range os.Args[1:] {
		if a == name {
			return true
		}
	}
	return false
}

func parseNonZeroInt(s string, def int) int {
	n, err := strconv.Atoi(strings.TrimSpace(s))
	if err != nil || n <= 0 {
		return def
	}
	return n
}

// dumpIDs prints every user and chat ID found in the bot's pending updates,
// then exits. Useful for discovering IDs before configuring the bot.
func dumpIDs(token string) {
	bot, err := gotgbot.NewBot(token, nil)
	if err != nil {
		log.Fatalf("create bot: %v", err)
	}
	updates, err := bot.GetUpdates(&gotgbot.GetUpdatesOpts{Limit: 100})
	if err != nil {
		log.Fatalf("get updates: %v", err)
	}
	users := map[int64]string{}
	chats := map[int64]string{}
	for _, u := range updates {
		if u.Message != nil {
			if u.Message.From != nil {
				users[u.Message.From.Id] = u.Message.From.FirstName
			}
			if u.Message.Chat.Id != 0 {
				chats[u.Message.Chat.Id] = u.Message.Chat.Type
			}
		}
	}
	fmt.Println("users:")
	for id, name := range users {
		fmt.Printf("  %d  %s\n", id, name)
	}
	fmt.Println("chats:")
	for id, typ := range chats {
		fmt.Printf("  %d  %s\n", id, typ)
	}
}

func main() {
	token := strings.TrimSpace(os.Getenv("TELEGRAM_BOT_TOKEN"))
	if token == "" {
		log.Fatal("TELEGRAM_BOT_TOKEN is required")
	}

	if hasArg("--getids") {
		dumpIDs(token)
		return
	}

	cfg := config{
		AdminIDs:   parseIDs(os.Getenv("ADMIN_USER_IDS")),
		SourceChat: parseChatID(os.Getenv("SOURCE_CHAT_ID")),
		TargetChat: parseChatID(os.Getenv("TARGET_CHAT_ID")),
		DataDir:    env("DATA_DIR", "data"),
		Token:      token,
		SaveMedia:  env("ASSISTANT_SAVE_MEDIA", "false") == "true",
		MaxMediaMB: parseNonZeroInt(env("ASSISTANT_MAX_MEDIA_MB", "50"), 50),
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
	dispatcher.AddHandler(handlers.NewCommand("ids", app.cmdIds))
	dispatcher.AddHandler(handlers.NewCommand("addkeyword", app.cmdAddKeyword))
	dispatcher.AddHandler(handlers.NewCommand("delkeyword", app.cmdDelKeyword))
	dispatcher.AddHandler(handlers.NewCommand("keywords", app.cmdKeywords))
	dispatcher.AddHandler(handlers.NewCommand("schedule", app.cmdSchedule))
	dispatcher.AddHandler(handlers.NewCommand("schedules", app.cmdSchedules))
	dispatcher.AddHandler(handlers.NewCommand("scheduledel", app.cmdScheduleDel))
	// assistant: watch channels + archive posts locally
	dispatcher.AddHandler(handlers.NewCommand("addchannel", app.cmdAddChannel))
	dispatcher.AddHandler(handlers.NewCommand("removechannel", app.cmdRemoveChannel))
	dispatcher.AddHandler(handlers.NewCommand("listchannels", app.cmdListChannels))
	dispatcher.AddHandler(handlers.NewCommand("asave", app.cmdASave))
	dispatcher.AddHandler(handlers.NewCommand("asearch", app.cmdASearch))
	dispatcher.AddHandler(handlers.NewCommand("aget", app.cmdAGet))
	dispatcher.AddHandler(handlers.NewCommand("astats", app.cmdAStats))
	dispatcher.AddHandler(handlers.NewCommand("aexport", app.cmdAExport))
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
