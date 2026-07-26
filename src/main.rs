mod app;
mod art;
mod config;
mod grep;
mod indexer;
mod markdown_parser;
mod state;
mod theme;
mod ui;

use crate::app::App;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;

fn cleanup() -> Result<(), Box<dyn std::error::Error>> {
    disable_raw_mode()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _panic_hook = std::panic::set_hook(Box::new(|info| {
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        let _ = disable_raw_mode();
        eprintln!("Panic: {}", info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    if let Some(arg) = std::env::args().nth(1) {
        let path = std::path::Path::new(&arg);
        if path.is_dir() {
            std::env::set_current_dir(&path)?;
        } else if let Some(parent) = path.parent() {
            if parent.exists() {
                std::env::set_current_dir(&parent)?;
            }
        }
    }

    let mut app = App::new();

    loop {
        app.try_recv_suggestions();
        app.try_recv_index_status();

        terminal.draw(|f| ui::draw(f, &mut app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == event::KeyEventKind::Release {
                continue;
            }

            let crossterm_event = app::KeyEvent {
                code: key.code,
                modifiers: key.modifiers,
            };

            if app.handle_key(&crossterm_event).is_some() {
                app.save_bookmarks();
                execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                cleanup()?;
                return Ok(());
            }
        }
    }
}
