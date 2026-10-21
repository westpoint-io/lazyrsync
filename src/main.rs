mod app;
mod editor;
mod headless;
mod history;
mod paths;
mod popups;
mod preview;
mod profile;
mod rsync;
mod run;
mod screens;
mod store;
mod ui;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "lazyrsync", version, about = "A terminal UI for rsync")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "Run a profile's tasks without the TUI")]
    Run {
        #[arg(
            value_name = "PROFILE[/TASK]",
            help = "Profile to run, or PROFILE/TASK for a single task (ids come from `list`)"
        )]
        target: String,

        #[arg(
            short = 'n',
            long,
            help = "Trial run: report what would change, change nothing"
        )]
        dry_run: bool,

        #[arg(
            long,
            help = "Allow tasks that delete files at the destination (not needed with -n)"
        )]
        yes: bool,

        #[arg(short = 'v', long, help = "Show rsync's full output for each task")]
        verbose: bool,
    },

    #[command(about = "List profiles, task ids, and their resolved rsync commands")]
    List,

    #[command(about = "Print recent runs, newest last")]
    Log {
        #[arg(short, long, default_value_t = 20, help = "How many runs to show")]
        limit: usize,
    },
}

const ERROR_LABEL: anstyle::Style = anstyle::Style::new()
    .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Red)))
    .bold();

fn config_error(e: anyhow::Error) -> ! {
    anstream::eprintln!("{ERROR_LABEL}error:{ERROR_LABEL:#} {e:#}");
    std::process::exit(2);
}

fn load_or_exit() -> store::Store {
    store::Store::load(false).unwrap_or_else(|e| config_error(e))
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::List) => {
            headless::list(&load_or_exit().profiles);
            Ok(())
        }
        Some(Command::Log { limit }) => std::process::exit(headless::log(limit)),
        Some(Command::Run {
            target,
            dry_run,
            yes,
            verbose,
        }) => {
            let store = load_or_exit();
            std::process::exit(headless::run(
                &store.profiles,
                &target,
                dry_run,
                yes,
                verbose,
            ));
        }
        None => {
            let mut app = app::App::new().unwrap_or_else(|e| config_error(e));
            let mut terminal = ratatui::init();
            let prev_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                let _ = crossterm::execute!(
                    std::io::stdout(),
                    crossterm::event::DisableBracketedPaste,
                    crossterm::event::DisableMouseCapture,
                );
                prev_hook(info);
            }));
            let _ = crossterm::execute!(
                std::io::stdout(),
                crossterm::event::EnableMouseCapture,
                crossterm::event::EnableBracketedPaste,
            );
            let result = app.run(&mut terminal);
            let _ = crossterm::execute!(
                std::io::stdout(),
                crossterm::event::DisableBracketedPaste,
                crossterm::event::DisableMouseCapture,
            );
            ratatui::restore();
            result
        }
    }
}
