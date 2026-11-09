//! workit: the Reddit frontpage, but every post is from Workplace.

mod app;
mod data;
mod ui;

use std::io;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event;
use ratatui::crossterm::event::DisableMouseCapture;
use ratatui::crossterm::event::EnableMouseCapture;
use ratatui::crossterm::event::Event;
use ratatui::crossterm::event::KeyEventKind;
use ratatui::crossterm::execute;

use crate::app::App;

fn run(terminal: &mut DefaultTerminal) -> io::Result<()> {
    let mut app = App::new();
    let mut redraw = true;
    while !app.should_quit {
        if redraw {
            terminal.draw(|frame| ui::render(frame, &mut app))?;
        }
        redraw = match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                app.on_key(key);
                true
            }
            Event::Mouse(mouse) if app.handles_mouse(&mouse) => {
                app.on_mouse(mouse);
                true
            }
            // Mostly pointer motion, a steady stream while the mouse moves.
            // These change nothing, so the frame on screen is still current.
            Event::Mouse(_) => false,
            _ => true,
        };
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    // ratatui's panic hook restores the terminal but knows nothing of mouse
    // capture, so turn that off first, then hand over to it.
    let restore = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(io::stdout(), DisableMouseCapture);
        restore(info);
    }));
    let result = execute!(io::stdout(), EnableMouseCapture).and_then(|()| run(&mut terminal));
    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}
