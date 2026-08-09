mod actions;
mod app;
mod config;
mod input;
mod keys;
mod keys_modal;
mod submit;
mod text;
mod tg;
mod ui;
mod ui_overlay;

use std::io;
use std::time::{Duration, Instant};

use anyhow::Result;
use app::App;
use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

const TICK: Duration = Duration::from_millis(120);

fn main() -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(run())
}

async fn run() -> Result<()> {
    let mut app = App::new()?;
    match (app.cfg.last_session.clone(), app.cfg.api_id) {
        (Some(session), Some(_)) => app.connect(session),
        (_, None) => app.begin_login(),
        _ => {}
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let res = event_loop(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    app.tg.disconnect();
    res
}

async fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    let mut last_tick = Instant::now();
    while !app.quit {
        app.pump();
        app.tick();
        terminal.draw(|f| ui::draw(f, app))?;

        // Redraw as soon as a key arrives; otherwise wake on the tick for the
        // spinner, toast expiry and task results.
        let wait = TICK.saturating_sub(last_tick.elapsed());
        if event::poll(wait)? {
            match event::read()? {
                Event::Key(key) => app.handle_key(key),
                Event::Resize(..) => terminal.autoresize()?,
                _ => {}
            }
        }
        if last_tick.elapsed() >= TICK {
            app.spinner = app.spinner.wrapping_add(1);
            last_tick = Instant::now();
        }
        tokio::task::yield_now().await;
    }
    Ok(())
}
