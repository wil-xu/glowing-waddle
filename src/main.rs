//! workit: the Reddit frontpage, but every post is from Workplace.

mod app;
mod data;
mod ui;

use std::io;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event;
use ratatui::crossterm::event::Event;
use ratatui::crossterm::event::KeyEventKind;

use crate::app::App;

fn run(terminal: &mut DefaultTerminal) -> io::Result<()> {
    let mut app = App::new();
    while !app.should_quit {
        terminal.draw(|frame| ui::render(frame, &mut app))?;
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.on_key(key);
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = run(&mut terminal);
    ratatui::restore();
    result
}
