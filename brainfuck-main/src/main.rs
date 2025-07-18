use clap::{Parser, Subcommand, Args, ValueEnum};
use std::fs;

use brainfuck_tui::{run_app, App, CrosstermTerminal};

/// CLI for processing and searching inputs
#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run input through the processor
    Run(RunArgs),

    /// Search input for a pattern
    Search(SearchArgs),

    /// launch TUI
    Tui,
}

#[derive(Args)]
struct RunArgs {
    /// Input string
    #[arg(short, long, required_unless_present = "file")]
    input: Option<String>,

    /// Path to input file
    #[arg(short, long, required_unless_present = "input")]
    file: Option<String>,
}

#[derive(Args)]
struct SearchArgs {
    /// Search target string
    #[arg(short, long, required_unless_present = "file")]
    target: Option<String>,

    /// Path to input file
    #[arg(short, long, required_unless_present = "target")]
    file: Option<String>,

    /// Input format
    #[arg(short, long, value_enum)]
    format: InputFormat,

    /// Enable multithreaded search
    #[arg(long)]
    multithread: bool,
}

#[derive(Clone, ValueEnum, Debug, Copy)]
enum InputFormat {
    Json,
    Xml,
    Txt,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run(args) => {
            let input = match args.input {
                Some(s) => s,
                None => {
                    fs::read_to_string(args.file.expect("Expected file")).expect("Failed to read file")
                }
            };
            run_handler(&input);
        }
        Commands::Search(args) => {
            let input = match args.target {
                Some(s) => s,
                None => {
                    fs::read_to_string(args.file.expect("Expected file")).expect("Failed to read file")
                }
            };
            search_handler(&input, args.format, args.multithread);
        }
        Commands::Tui => {
            let mut terminal = CrosstermTerminal::new().expect("Failed to create terminal");
            let mut app = App::new();
            run_app(&mut terminal, &mut app).expect("Failed to run TUI app");
            terminal.try_close().expect("Failed to close terminal");
        }
    }
}

fn run_handler(input: &str) {
    println!("Running with input:\n{}", input);
    // Your actual logic here
}

fn search_handler(input: &str, format: InputFormat, multithread: bool) {
    println!("Searching in format {:?} with multithread: {}", format, multithread);
    println!("Input:\n{}", input);
    // Your actual logic here
}
