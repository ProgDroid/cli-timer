use clap::Parser;
use cli_timer::app::{App, Args, Result};
use cli_timer::event::{Event, Handler};
use cli_timer::handler::handle_key_events;
use cli_timer::tui::Tui;
use std::io;
use tui::backend::CrosstermBackend;
use tui::Terminal;

fn main() -> Result<()> {
    let args = Args::parse();

    let mut app = App::new(args);

    let backend = CrosstermBackend::new(io::stderr());
    let terminal = Terminal::new(backend)?;
    let events = Handler::new(250);
    let mut tui = Tui::new(terminal, events);
    tui.init()?;

    while app.running {
        tui.draw(&mut app)?;

        match tui.events.next()? {
            Event::Tick => app.tick(),
            Event::Key(key_event) => handle_key_events(key_event, &mut app)?,
            Event::Mouse(_) | Event::Resize(_, _) => {}
        }
    }

    tui.exit()?;
    Ok(())
}
