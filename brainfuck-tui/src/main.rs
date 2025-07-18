use std::{error::Error, io};

use brainfuck_tui::{run_app, App, CrosstermTerminal};
use ratatui::{
    crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{
            disable_raw_mode,
            enable_raw_mode,
            EnterAlternateScreen,
            LeaveAlternateScreen,
        },
    },
    prelude::*,
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut terminal = CrosstermTerminal::new()?;

    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app);

    terminal.try_close()?;

    res.map(|_| ())
}