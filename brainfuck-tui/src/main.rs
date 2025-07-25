use std::error::Error;

use brainfuck_tui::{App, CrosstermTerminal, run_app};

fn main() -> Result<(), Box<dyn Error>> {
    let mut terminal = CrosstermTerminal::new()?;

    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app);

    terminal.try_close()?;

    res.map(|_| ())
}
