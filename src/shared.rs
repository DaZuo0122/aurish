use tui_input::Input;
use ratatui::prelude::*;
use ishell::IShell;
use ratatui::{
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
        execute,
        terminal::{
            disable_raw_mode, enable_raw_mode, EnterAlternateScreen,
            LeaveAlternateScreen,
        },
    },
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use std::{error::Error, io};
use ratatui::text::Line;
use tui_input::backend::crossterm::EventHandler;
use serde::{Serialize, Deserialize};
use std::env::current_dir;
use std::path::PathBuf;
use std::collections::VecDeque;

pub enum EditMode {
    Input,  // In this mode, user interact with input box
    Normal,  // This is the default mode, where user can exit or start editing
    Shell,  // In this mode, user interact with spawned shell
}

pub struct App {
    input: Input,
    input_mode: EditMode,
    messages: Vec<String>,
    shell_commands: VecDeque<String>,
    shell: DummyShell,
}

pub struct DummyShell {
    curr_path: PathBuf,
    shell: IShell,
    pub pending_command: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    ollama_api: String,
    model: String,
    proxy: String,
}

impl Default for App {
    fn default() -> Self {
        App {
            input: Input::default(),
            input_mode: EditMode::Normal,
            messages: Vec::new(),
            shell_commands: VecDeque::new(),
            shell: DummyShell::default(),
        }
    }
}

impl Default for DummyShell {
    fn default() -> Self {
        DummyShell {
            curr_path: current_dir().unwrap(),
            shell: IShell::new(),
            pending_command: String::new(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            ollama_api: String::from("http://localhost:11434/api/generate"),
            model: String::from("llama3:latest"),
            proxy: String::from(""),
        }
    }
}

impl DummyShell {
    pub fn renew_path(&mut self) {
        self.curr_path = current_dir().unwrap();
    }

    pub fn get_path(&self) -> String {
        /// Showing current path like actual Shell did
        let path = self.curr_path.to_string_lossy().into_owned();
        path
    }
}

impl Config {
    pub fn set_proxy(&mut self, proxy: String) {
        self.proxy = proxy;
    }

    pub fn set_ollama_api(&mut self, api: String) {
        self.ollama_api = api;
    }

    pub fn set_model(&mut self, model: String) {
        self.model = model;
    }

    pub fn get_model(&self) -> &str {
        self.model.as_str()
    }

    pub fn get_ollama_api(&self) -> &str {
        self.ollama_api.as_str()
    }

    pub fn get_proxy(&self) -> &str {
        self.proxy.as_str()
    }

    /// Check whether proxy in Config is set
    pub fn uses_proxy(&self) -> bool {
        if self.proxy == "".to_string() {
            false
        } else { true }
    }
}

impl App {
    fn ui(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints(
                [
                    Constraint::Length(1),
                    Constraint::Length(3),
                    Constraint::Min(1),
                ].as_ref(),
            )
            .split(frame.area());

        let (msg, style) = match self.input_mode {
            EditMode::Normal => (
                vec![
                    Span::raw("Press "),
                    Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" to exit, "),
                    Span::styled("a", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" to ask AI, "),
                    Span::styled("s", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" to interact with Shell."),
                ],
                Style::default().add_modifier(Modifier::RAPID_BLINK),
            ),
            EditMode::Input => (
                vec![
                    Span::raw("Press "),
                    Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" stop asking AI, "),
                    Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" to send the message"),
                ],
                Style::default(),
            ),
            EditMode::Shell => (
                vec![
                    Span::raw("Press "),
                    Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" stop Shell interaction, "),
                    Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" to execute shell command"),
                ],
                Style::default(),
            ),
        };
        let text = Text::from(Line::from(msg)).style(style);
        let help_msg = Paragraph::new(text);
        frame.render_widget(help_msg, chunks[0]);

        let width = chunks[0].width.max(3) - 3;  // 2 for boarders and 1 for cursor
        let scroll = self.input.visual_scroll(width as usize);
        let input = Paragraph::new(self.input.value())
            .style(match self.input_mode {
                EditMode::Normal => Style::default(),
                EditMode::Input => Style::default().fg(Color::Yellow),
                EditMode::Shell => Style::default().fg(Color::Blue),
            })
            .scroll((0, scroll as u16))
            .block(Block::default().borders(Borders::ALL).title("Asking AI"));
        frame.render_widget(input, chunks[1]);
        let (msg, comm_len) = self.get_lo_msg();
        frame.render_widget(msg, chunks[2]);
        // let lower_msg = List::new(self.shell.pending_command);
        match self.input_mode {
            EditMode::Normal => {},
            // Hide cursor in normal mode
            EditMode::Input => {
                frame.set_cursor_position((
                    chunks[1].x
                        + (self.input.visual_cursor().max(scroll) - scroll) as u16
                        + 1,
                    chunks[1].y + 1
                ))
            },
            EditMode::Shell => {
                let start_pos = self.shell.get_path().len();
                frame.set_cursor_position((
                    chunks[2].x
                        + (self.input.visual_cursor().max(scroll + start_pos + comm_len) - scroll - start_pos) as u16
                        + 1,
                    chunks[2].y + 1
                ));
            }
        }
    }

    fn get_lo_msg(&mut self) -> (List, usize) {
        /// Return a List for render and length of command
        let command = self.shell_commands.pop_front().unwrap();
        self.shell.pending_command = command.clone();
        self.shell.renew_path();
        let mut path = self.shell.get_path();
        let msg = path.push_str(&self.shell.pending_command);
        let msg_list = List::new([path])
            .block(Block::default().borders(Borders::ALL).title("Shell"))
            .highlight_style(Style::new())
            .highlight_symbol(">>")
            .repeat_highlight_symbol(true);
        (msg_list, command.len())
    }
}
