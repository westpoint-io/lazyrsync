mod app;
mod editor;
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
    Run {
        profile: String,

        #[arg(short = 'n', long)]
        dry_run: bool,
    },

    List,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::List) => {
            let store = store::Store::load()?;
            if store.profiles.is_empty() {
                println!("No profiles. Add one in the TUI (run `lazyrsync`).");
            }
            for p in &store.profiles {
                println!("{}", p.name);
                for t in &p.tasks {
                    println!("  {}", t.id);
                    println!("      {}", rsync::resolved_command(t, false));
                }
                println!();
            }
            Ok(())
        }
        Some(Command::Run { profile, dry_run }) => {
            let store = store::Store::load()?;
            let p = store
                .profiles
                .iter()
                .find(|p| p.name == profile)
                .ok_or_else(|| anyhow::anyhow!("no profile named '{profile}'"))?;
            for t in &p.tasks {
                println!("{}", rsync::resolved_command(t, dry_run));
            }
            eprintln!("(execution engine not wired yet — command shown above)");
            Ok(())
        }
        None => {
            let mut app = app::App::new()?;
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
