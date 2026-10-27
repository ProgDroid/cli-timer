use clap::error::ErrorKind;
use clap::{CommandFactory, Parser};
use cli_timer::alert;
use cli_timer::app::{App, Result};
use cli_timer::cli::{Cli, Command, TuiArgs};
use cli_timer::ding;
use cli_timer::event::{Event, Handler};
use cli_timer::handler::handle_key_events;
use cli_timer::remind;
use cli_timer::sound;
use cli_timer::tui::Tui;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Remind(args)) => {
            ExitCode::from(remind::run(args, &*alert::from_env(), &mut io::stdout()))
        }
        Some(Command::Ding(args)) => {
            ExitCode::from(ding::run(args, &*alert::from_env(), &mut io::stdout()))
        }
        None => match run_timer(cli.tui) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("Error: {e}");
                ExitCode::FAILURE
            }
        },
    }
}

/// The full-screen timer; without `-t` it is a usage error (exit 2), like any missing argument.
fn run_timer(args: TuiArgs) -> Result<()> {
    let Some(time) = args.time else {
        Cli::command()
            .error(
                ErrorKind::MissingRequiredArgument,
                "the following required arguments were not provided:\n  -t <TIME>",
            )
            .exit()
    };

    let mut app = App::new(time, args.sound, args.label)?;
    // Before the screen opens: once it has, an error would be drawn over and lost.
    sound::check(&app.sound)?;

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
