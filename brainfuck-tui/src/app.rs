use core::panic;
use std::error::Error;

use ratatui::crossterm::event::{self, Event as CEvent, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::*;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use tui_scrollview::{ScrollView, ScrollViewState};

use crate::raw_terminal::RawTerminal;

#[derive(Default, Clone, Debug)]
struct InputEntry {
    bytes: Vec<u8>,
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
enum Mode {
    EditAscii,
    EditDec,
    EditHex,
    Normal,
    Running,
}

#[derive(Debug)]
pub struct App {
    inputs: Vec<InputEntry>,
    selected_input: usize,
    mode: Mode,
    edit_cursor: usize, //this is the character index in the current input entry being edited
    digit_cursor: usize, //this is the digit index in the current input entry being edited
    scroll_state: ScrollViewState,
    calculated_current_layout: Vec<Rect>, // stores the positions of each input entry in the layout
    input_display_area: Option<Rect>,
    copy_buffer: Option<Vec<u8>>,
}

#[allow(unused)]
enum Direction {
    Up,
    Left,
    Down,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
}

impl App {
    pub fn new() -> Self {
        Self {
            inputs: vec![InputEntry {
                bytes: vec![0u8; 4],
            }],
            selected_input: 0,
            mode: Mode::Normal,
            edit_cursor: 0,
            digit_cursor: 0,
            scroll_state: ScrollViewState::default(),
            calculated_current_layout: vec![],
            input_display_area: None,
            copy_buffer: None,
        }
    }

    fn draw(&mut self, f: &mut Frame) {
        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(f.area());

        let controls_text = match self.mode {
            Mode::Normal => {
                "Normal: 'a' add, 'd' delete, 'e' edit, 'r' run, arrows navigate, Ctrl+C copy, Ctrl+V paste, Ctrl+X cut, 'q' quit"
            }
            Mode::EditAscii => {
                "Edit ASCII: Type chars  , +/- resize, Shift+arrows move cursor/mode, Ctrl+arrows inc/dec value, Esc exit"
            }
            Mode::EditDec => {
                "Edit DEC: Type digits     , +/- resize, Shift+arrows move cursor/mode, Ctrl+arrows inc/dec value, Esc exit"
            }
            Mode::EditHex => {
                "Edit HEX: Type hex digits , +/- resize, Shift+arrows move cursor/mode, Ctrl+arrows inc/dec value, Esc exit"
            }
            Mode::Running => "Running: Press any key to return to Normal mode",
        };

        f.render_widget(
            Paragraph::new(controls_text)
                .block(Block::default().borders(Borders::ALL).title("Controls"))
                .wrap(Wrap { trim: true }),
            chunks[0],
        );

        let inputs_area = chunks[1];

        let grid_width = inputs_area.width - 1;
        self.calculate_layout_height(grid_width);

        let layout_height = self
            .calculated_current_layout
            .iter()
            .map(|rect| rect.bottom())
            .max()
            .unwrap_or(0);

        let mut scroll_view = ScrollView::new(Size {
            width: inputs_area.width,
            height: layout_height,
        })
        .vertical_scrollbar_visibility(tui_scrollview::ScrollbarVisibility::Automatic)
        .horizontal_scrollbar_visibility(tui_scrollview::ScrollbarVisibility::Never);

        for (i, entry) in self.inputs.iter().enumerate() {
            let render_line = |label: &str,
                               values: &[String],
                               highlight: bool,
                               selected_idx: usize,
                               digit_cursor: usize| {
                let mut line: Vec<Span<'static>> = vec![
                    Span::styled(label.to_string(), Style::default().fg(Color::Gray)),
                    Span::raw(" ".to_string()),
                ];

                for (j, val) in values.iter().enumerate() {
                    let required_spacing = 5 - val.chars().count();
                    let mut style = Style::default().fg(Color::White).bg(Color::Reset);
                    if highlight && j == selected_idx {
                        style = style.bg(Color::DarkGray).add_modifier(Modifier::BOLD);
                    }

                    // Highlight the digit at digit_cursor differently
                    if highlight && j == selected_idx {
                        let chars: Vec<char> = val.chars().collect();
                        let mut spans = Vec::new();
                        for (idx, ch) in chars.iter().enumerate() {
                            if idx == digit_cursor {
                                // Special style for digit_cursor
                                spans.push(Span::styled(
                                    ch.to_string(),
                                    style
                                        .bg(Color::Blue)
                                        .fg(Color::Yellow)
                                        .add_modifier(Modifier::REVERSED),
                                ));
                            } else {
                                spans.push(Span::styled(ch.to_string(), style));
                            }
                        }
                        line.extend(spans);
                    } else {
                        line.push(Span::styled(val.clone(), style));
                    }

                    line.push(Span::styled(" ".repeat(required_spacing), style));
                }

                Line::from(line)
            };

            let ascii: Vec<String> = entry
                .bytes
                .iter()
                .map(|b| {
                    if b.is_ascii_graphic() {
                        format!("{}", *b as char)
                    } else {
                        match b {
                            b'\n' => "\\n".to_string(),
                            b'\r' => "\\r".to_string(),
                            b'\t' => "\\t".to_string(),
                            0 => "\\0".to_string(),
                            _ => "\u{25AF}".to_string(), // Unicode replacement character
                        }
                    }
                })
                .collect();
            let dec: Vec<String> = entry.bytes.iter().map(|b| format!("{:03}", b)).collect();

            let hex: Vec<String> = entry.bytes.iter().map(|b| format!("{:02X}", b)).collect();

            let is_selected = self.selected_input == i;

            let total = Text::from(vec![
                render_line(
                    "ASCII:",
                    &ascii,
                    self.mode == Mode::EditAscii && is_selected,
                    self.edit_cursor,
                    self.digit_cursor,
                ),
                render_line(
                    "DEC:  ",
                    &dec,
                    self.mode == Mode::EditDec && is_selected,
                    self.edit_cursor,
                    self.digit_cursor,
                ),
                render_line(
                    "HEX:  ",
                    &hex,
                    self.mode == Mode::EditHex && is_selected,
                    self.edit_cursor,
                    self.digit_cursor,
                ),
            ]);

            scroll_view.render_widget(
                Paragraph::new(total).block(
                    Block::default()
                        .title(format!("input #{i}"))
                        .borders(Borders::all())
                        .border_type(if is_selected {
                            BorderType::Double
                        } else {
                            BorderType::Plain
                        }),
                ),
                self.calculated_current_layout[i],
            );
        }

        f.render_stateful_widget(scroll_view, inputs_area, &mut self.scroll_state);
        self.input_display_area = Some(inputs_area);

        f.render_widget(
            Paragraph::new(format!("Mode: {:?}", self.mode))
                .block(Block::default().borders(Borders::NONE)),
            chunks[2],
        );
    }

    fn calculate_layout_height(&mut self, grid_width: u16) {
        let mut current_grid_x = 0;
        let mut current_grid_y = 0;
        self.calculated_current_layout.clear();
        for entry in &self.inputs {
            let required_width = 7 + 5 * entry.bytes.len() as u16;
            if current_grid_x + required_width > grid_width {
                current_grid_x = 0;
                current_grid_y += 1;
            }
            self.calculated_current_layout.push(Rect::new(
                current_grid_x,
                5 * current_grid_y,
                required_width,
                5,
            ));
            current_grid_x += required_width;
        }
    }

    fn find_closest(
        &self,
        current_area: Rect,
        out_from_dir: Direction,
        search_dir: Direction,
    ) -> Option<usize> {
        let out_from = match out_from_dir {
            Direction::Up => (0, -1),
            Direction::Down => (0, 1),
            Direction::Left => (-1, 0),
            Direction::Right => (1, 0),
            Direction::UpLeft => (-1, -1),
            Direction::UpRight => (1, -1),
            Direction::DownLeft => (-1, 1),
            Direction::DownRight => (1, 1),
        };

        let search_to = match search_dir {
            Direction::Up => (0, -1),
            Direction::Down => (0, 1),
            Direction::Left => (-1, 0),
            Direction::Right => (1, 0),
            Direction::UpLeft => (-1, -1),
            Direction::UpRight => (1, -1),
            Direction::DownLeft => (-1, 1),
            Direction::DownRight => (1, 1),
        };

        // start just outside current_area based on out_from direction
        let mid_x = current_area.left() as i32 + (current_area.width as i32 / 2);
        let mid_y = current_area.top() as i32 + (current_area.height as i32 / 2);
        let mut x = mid_x;
        let mut y = mid_y;
        if out_from.0 != 0 {
            while current_area.contains(Position::new(x as u16, mid_y as u16)) {
                x += out_from.0;
            }
        }
        if out_from.1 != 0 {
            while current_area.contains(Position::new(mid_x as u16, y as u16)) {
                y += out_from.1;
            }
        }
        loop {
            let pos = Position::new(x as u16, y as u16);
            for (i, area) in self.calculated_current_layout.iter().enumerate() {
                if area.contains(pos) {
                    return Some(i);
                }
            }
            let next_x = x + search_to.0;
            let next_y = y + search_to.1;
            if next_x < 0 || next_y < 0 {
                break;
            }
            if next_x
                >= self
                    .calculated_current_layout
                    .iter()
                    .map(|r| r.right())
                    .max()
                    .unwrap_or(0) as i32
                || next_y
                    >= self
                        .calculated_current_layout
                        .iter()
                        .map(|r| r.bottom())
                        .max()
                        .unwrap_or(0) as i32
            {
                break;
            }
            x = next_x;
            y = next_y;
        }
        None
    }

    //make sure the currently selected input is visible in the scroll view
    fn adjust_scroll(&mut self) {
        // The area of the selected widget (in buffer‐space coords)
        let target_area = self.calculated_current_layout[self.selected_input];

        let mut current_scroll = self.scroll_state.offset();
        let top_seen = current_scroll.y;
        let bottom_seen = current_scroll.y + self.input_display_area.unwrap().height;

        if top_seen > target_area.top() {
            // Scroll up to make the top of the area visible
            current_scroll = Position::new(current_scroll.x, target_area.top());
        } else if bottom_seen < target_area.bottom() {
            // Scroll down to make the bottom of the area visible
            current_scroll = Position::new(
                current_scroll.x,
                target_area.bottom() - self.input_display_area.unwrap().height,
            );
        }

        self.scroll_state.set_offset(current_scroll);
    }

    fn handle_event(&mut self, ev: CEvent) -> bool {
        if let CEvent::Key(key) = ev {
            if key.kind == KeyEventKind::Press {
                match self.mode {
                    Mode::Normal => match key.code {
                        KeyCode::Char('q') => return false,
                        KeyCode::Char('a') => self.inputs.push(InputEntry {
                            bytes: vec![0u8; 1],
                        }),
                        KeyCode::Char('d') => {
                            if self.inputs.len() > 1 {
                                self.inputs.remove(self.selected_input);
                                if self.selected_input >= self.inputs.len() {
                                    self.selected_input = self.inputs.len() - 1;
                                }
                            }
                        }
                        KeyCode::Char('e') => {
                            self.mode = Mode::EditAscii;
                            self.edit_cursor = 0;
                        }
                        KeyCode::Char('r') => self.mode = Mode::Running,
                        KeyCode::Up => {
                            if let Some(idx) = self.find_closest(
                                self.calculated_current_layout[self.selected_input],
                                Direction::Up,
                                Direction::Left,
                            ) {
                                self.selected_input = idx;
                                self.adjust_scroll();
                            }
                        }
                        KeyCode::Down => {
                            if let Some(idx) = self.find_closest(
                                self.calculated_current_layout[self.selected_input],
                                Direction::Down,
                                Direction::Left,
                            ) {
                                self.selected_input = idx;
                                self.adjust_scroll();
                            }
                        }
                        KeyCode::Left => {
                            if let Some(idx) = self.find_closest(
                                self.calculated_current_layout[self.selected_input],
                                Direction::Left,
                                Direction::Left,
                            ) {
                                self.selected_input = idx;
                            }
                        }
                        KeyCode::Right => {
                            if let Some(idx) = self.find_closest(
                                self.calculated_current_layout[self.selected_input],
                                Direction::Right,
                                Direction::Right,
                            ) {
                                self.selected_input = idx;
                            }
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            // Clear the selected input
                            self.copy_buffer = Some(self.inputs[self.selected_input].bytes.clone());
                        }
                        KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            // Paste the copied bytes into a new input entry at the current position
                            if let Some(buffer) = &self.copy_buffer {
                                self.inputs.insert(
                                    self.selected_input + 1,
                                    InputEntry {
                                        bytes: buffer.clone(),
                                    },
                                );
                                self.selected_input += 1;
                            }
                        }
                        KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            self.copy_buffer = Some(self.inputs[self.selected_input].bytes.clone());
                            if self.inputs.len() > 1 {
                                self.inputs.remove(self.selected_input);
                                if self.selected_input >= self.inputs.len() {
                                    self.selected_input = self.inputs.len() - 1;
                                }
                            }
                        }

                        _ => {}
                    },
                    mode @ (Mode::EditAscii | Mode::EditDec | Mode::EditHex) => match key.code {
                        KeyCode::Char('+') => {
                            self.inputs[self.selected_input].bytes.push(0);
                        }
                        KeyCode::Char('-') => {
                            if !self.inputs[self.selected_input].bytes.len() > 1 {
                                self.inputs[self.selected_input].bytes.pop();
                            }
                        }
                        KeyCode::Char(c) => {
                            if let Mode::EditAscii = self.mode {
                                if c.is_ascii() {
                                    self.inputs[self.selected_input].bytes[self.edit_cursor] =
                                        c as u8;
                                }
                            }
                            if let Mode::EditDec = self.mode {
                                if c.is_ascii_digit() {
                                    //get the current digits of the value
                                    let prev_value = self.inputs[self.selected_input].bytes
                                        [self.edit_cursor]
                                        as usize;
                                    let new_digit_value = c.to_digit(10).unwrap() as usize;
                                    let old_hundreds = prev_value / 100;
                                    let old_tens = (prev_value / 10) % 10;
                                    let old_units = prev_value % 10;
                                    let new_value = match self.digit_cursor {
                                        0 => new_digit_value * 100 + old_tens * 10 + old_units,
                                        1 => old_hundreds * 100 + new_digit_value * 10 + old_units,
                                        2 => old_hundreds * 100 + old_tens * 10 + new_digit_value,
                                        _ => prev_value,
                                    };

                                    if new_value < 256 {
                                        self.inputs[self.selected_input].bytes[self.edit_cursor] =
                                            new_value as u8;
                                    } else {
                                        self.inputs[self.selected_input].bytes[self.edit_cursor] =
                                            0xFF;
                                    }
                                }
                            }
                            if let Mode::EditHex = self.mode {
                                if c.is_ascii_hexdigit() {
                                    let prev_value =
                                        self.inputs[self.selected_input].bytes[self.edit_cursor];
                                    let new_digit_value = c.to_digit(16).unwrap() as u8;
                                    let new_value = match self.digit_cursor {
                                        0 => (prev_value & 0x0F) | (new_digit_value << 4),
                                        1 => (prev_value & 0xF0) | new_digit_value,
                                        _ => prev_value,
                                    };

                                    self.inputs[self.selected_input].bytes[self.edit_cursor] =
                                        new_value;
                                }
                            }
                        }
                        KeyCode::Left if key.modifiers.contains(KeyModifiers::SHIFT) => {
                            if self.edit_cursor > 0 {
                                self.edit_cursor -= 1;
                            }
                        }
                        KeyCode::Right if key.modifiers.contains(KeyModifiers::SHIFT) => {
                            let len = self.inputs[self.selected_input].bytes.len();
                            if self.edit_cursor + 1 < len {
                                self.edit_cursor += 1;
                            }
                        }
                        KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
                            self.mode = match self.mode {
                                Mode::EditAscii => Mode::EditHex,
                                Mode::EditHex => Mode::EditDec,
                                Mode::EditDec => Mode::EditAscii,
                                _ => self.mode,
                            };
                        }
                        KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                            self.mode = match self.mode {
                                Mode::EditAscii => Mode::EditDec,
                                Mode::EditDec => Mode::EditHex,
                                Mode::EditHex => Mode::EditAscii,
                                _ => self.mode,
                            };
                        }
                        KeyCode::Up if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            self.inputs[self.selected_input].bytes[self.edit_cursor] =
                                self.inputs[self.selected_input].bytes[self.edit_cursor]
                                    .saturating_add(1);
                        }
                        KeyCode::Down if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            self.inputs[self.selected_input].bytes[self.edit_cursor] =
                                self.inputs[self.selected_input].bytes[self.edit_cursor]
                                    .saturating_sub(1);
                        }
                        KeyCode::Left => {
                            if self.digit_cursor > 0 {
                                self.digit_cursor -= 1;
                            }
                        }
                        KeyCode::Right => {
                            let max = match mode {
                                Mode::EditAscii => 0,
                                Mode::EditDec => 2,
                                Mode::EditHex => 1,
                                _ => panic!("Invalid mode for left cursor movement"),
                            };
                            if self.digit_cursor < max {
                                self.digit_cursor += 1;
                            }
                        }
                        KeyCode::Up => {
                            self.digit_cursor = 0;
                        }
                        KeyCode::Down => {
                            self.digit_cursor = match mode {
                                Mode::EditAscii => 0,
                                Mode::EditDec => 2,
                                Mode::EditHex => 1,
                                _ => panic!("Invalid mode for down cursor movement"),
                            };
                        }

                        KeyCode::Esc => self.mode = Mode::Normal,
                        _ => {}
                    },
                    Mode::Running => {
                        self.mode = Mode::Normal;
                    }
                }
            }
        }
        true
    }
}

pub fn run_app<T: RawTerminal>(terminal: &mut T, app: &mut App) -> Result<bool, Box<dyn Error>> {
    loop {
        terminal.draw(|f| app.draw(f))?;
        if event::poll(std::time::Duration::from_millis(250))?
            && !app.handle_event(event::read()?)
        {
            return Ok(false);
        }
    }
}

