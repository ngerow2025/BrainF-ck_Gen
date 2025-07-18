use std::io;

use ratatui::{
    CompletedFrame, Frame, Terminal,
    crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
    prelude::CrosstermBackend,
};

pub trait RawTerminal {
    fn draw<T>(&mut self, render_callback: T) -> io::Result<CompletedFrame>
    where
        T: FnOnce(&mut Frame);
}

pub struct CrosstermTerminal {
    inner: Terminal<CrosstermBackend<std::io::Stdout>>,
    closed: bool,
}

impl CrosstermTerminal {
    pub fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(CrosstermTerminal {
            inner: terminal,
            closed: false,
        })
    }

    pub fn try_close(&mut self) -> io::Result<()> {
        if !self.closed {
            disable_raw_mode()?;
            execute!(
                self.inner.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )?;
            self.inner.show_cursor()?;
            self.closed = true;
        }
        Ok(())
    }
}

impl Drop for CrosstermTerminal {
    fn drop(&mut self) {
        self.try_close().unwrap();
    }
}

impl RawTerminal for CrosstermTerminal {
    fn draw<T>(&mut self, render_callback: T) -> io::Result<CompletedFrame>
    where
        T: FnOnce(&mut Frame),
    {
        self.inner.draw(render_callback)
    }
}
