package main

import (
	"log"
	"time"
)

// runScheduler periodically delivers due scheduled messages and persists progress.
func (a *app) runScheduler() {
	ticker := time.NewTicker(5 * time.Second)
	defer ticker.Stop()
	for range ticker.C {
		a.tickScheduler()
	}
}

func (a *app) tickScheduler() {
	now := time.Now().Unix()
	a.mu.Lock()
	due := make([]scheduleItem, 0)
	for i := range a.schedules {
		if now-a.schedules[i].Last >= a.schedules[i].Interval {
			a.schedules[i].Last = now
			due = append(due, a.schedules[i])
		}
	}
	a.mu.Unlock()
	if len(due) == 0 {
		return
	}
	for _, s := range due {
		if _, err := a.bot.SendMessage(s.ChatID, s.Text, nil); err != nil {
			log.Printf("schedule #%d: %v", s.ID, err)
			// do not persist a failed send as delivered; try again next tick
			a.mu.Lock()
			for i := range a.schedules {
				if a.schedules[i].ID == s.ID && a.schedules[i].Last == now {
					a.schedules[i].Last = now - s.Interval
				}
			}
			a.mu.Unlock()
		}
	}
	_ = a.save("schedules.json", a.schedules)
}
