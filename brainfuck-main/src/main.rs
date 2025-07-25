use brainfuck_core::{run_program_fragment_no_target, util::preprocess_input};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::fs;

use brainfuck_tui::{App, CrosstermTerminal, run_app};

/// CLI for processing and searching inputs
#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run input through the Brainfuck interpreter.
    ///
    /// This uses all optimizations including jump tables, therefore requiring a full preprocessing pass of the input.
    /// Default memory size is 30,000 cells, does not automatically resize, and throws errors if the program attempts to move pointer out of bounds in either direction.
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
                None => fs::read_to_string(args.file.expect("Expected file"))
                    .expect("Failed to read file"),
            };
            run_code(&input);
        }
        Commands::Search(args) => {
            let input = match args.target {
                Some(s) => s,
                None => fs::read_to_string(args.file.expect("Expected file"))
                    .expect("Failed to read file"),
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

fn run_code(input: &str) {
    let preprocessed_code = preprocess_input(input);
    match preprocessed_code {
        Ok(running_program_info) => {
            run_program_fragment_no_target(
                &running_program_info,
                || None,
                |output| {
                    print!("{}", output as char);
                },
            );
        }
        Err(e) => {
            eprintln!("Error preprocessing input: {}", e);
        }
    }
}

fn search_handler(input: &str, format: InputFormat, multithread: bool) {
    println!(
        "Searching in format {:?} with multithread: {}",
        format, multithread
    );
    println!("Input:\n{}", input);
    // Your actual logic here
}
