pub mod markdown_parser;
pub mod theme;
pub mod ui;
pub mod kitty_preview;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use ratatui_image::picker::Picker;
use std::{
    io,
    sync::Arc,
    time::Duration,
};
use texp_core::app::App;
use texp_core::event::AppEvent;

fn translate_key(code: KeyCode, modifiers: KeyModifiers) -> Option<AppEvent> {
    match (code, modifiers) {
        (KeyCode::Up, _) => Some(AppEvent::Up),
        (KeyCode::Down, _) => Some(AppEvent::Down),
        (KeyCode::Left, KeyModifiers::ALT) => Some(AppEvent::AltLeft),
        (KeyCode::Right, KeyModifiers::ALT) => Some(AppEvent::AltRight),
        (KeyCode::Left, m) if m.contains(KeyModifiers::CONTROL) && m.contains(KeyModifiers::SHIFT) => Some(AppEvent::CtrlShiftLeft),
        (KeyCode::Right, m) if m.contains(KeyModifiers::CONTROL) && m.contains(KeyModifiers::SHIFT) => Some(AppEvent::CtrlShiftRight),
        (KeyCode::Left, KeyModifiers::CONTROL) => Some(AppEvent::CtrlLeft),
        (KeyCode::Right, KeyModifiers::CONTROL) => Some(AppEvent::CtrlRight),
        (KeyCode::Left, _) => Some(AppEvent::Left),
        (KeyCode::Right, _) => Some(AppEvent::Right),
        (KeyCode::Enter, _) => Some(AppEvent::Enter),
        (KeyCode::Esc, _) => Some(AppEvent::Escape),
        (KeyCode::Backspace, _) => Some(AppEvent::Backspace),
        (KeyCode::Tab, _) => Some(AppEvent::Tab),
        (KeyCode::Delete, _) => Some(AppEvent::Delete),
        (KeyCode::Home, _) => Some(AppEvent::Home),
        (KeyCode::End, _) => Some(AppEvent::End),
        (KeyCode::PageUp, _) => Some(AppEvent::PageUp),
        (KeyCode::PageDown, _) => Some(AppEvent::PageDown),
        (KeyCode::F(n), _) => Some(AppEvent::F(n)),
        (KeyCode::Char(c), m) if m == KeyModifiers::CONTROL => Some(AppEvent::Ctrl(c)),
        (KeyCode::Char(c), _) => Some(AppEvent::Char(c)),
        _ => None,
    }
}

fn cleanup() -> Result<(), Box<dyn std::error::Error>> {
    disable_raw_mode()?;
    Ok(())
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
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

    let appearance = theme::AppearanceConfig::load();

    let picker = Arc::new(
        Picker::from_query_stdio()
            .unwrap_or_else(|_| Picker::halfblocks()),
    );
    let mut preview = kitty_preview::PreviewModule::new(picker);

    loop {
        app.try_recv_index_status();

        terminal.draw(|f| ui::draw(f, &mut app, &appearance, &mut preview))?;

        if event::poll(Duration::from_millis(150))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Release {
                    continue;
                }

                if let Some(app_event) = translate_key(key.code, key.modifiers) {
                    if app.handle_event(&app_event).is_some() {
                        app.save_bookmarks();
                        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                        cleanup()?;
                        return Ok(());
                    }
                }
            }
        }
    }
}
