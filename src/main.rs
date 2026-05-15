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
