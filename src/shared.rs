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
    DefaultTerminal, Frame,
};
use std::{error::Error, io};
use std::any::TypeId;
use std::cell::RefCell;
use std::rc::Rc;
use ratatui::text::Line;
use tui_input::backend::crossterm::EventHandler;
use serde::{Serialize, Deserialize};
use std::env::current_dir;
use std::path::PathBuf;
use std::collections::VecDeque;
use crate::backend::{Bclient, OllamaReq};

pub enum EditMode {
    Input,  // In this mode, user interact with input box
    Normal,  // This is the default mode, where user can exit or start editing
    Shell,  // In this mode, user interact with spawned shell
}

pub struct App {
    /// Current value of input box
    input: Input,
    input_mode: EditMode,
    messages: OllamaReq,
    /// Shell commands from LLM
    shell_commands: VecDeque<String>,
    shell: DummyShell,
}

pub struct DummyShell {
    curr_path: PathBuf,
    shell: IShell,
    executed_command: String,
    current_command: String,
    sh_input: Rc<RefCell<Input>>,
    sh_output: String,
    executed: bool,
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
            messages: OllamaReq::new("llama3:latest"),
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
            executed_command: String::new(),
            current_command: String::new(),
            sh_input: Rc::new(RefCell::new(Input::default())),
            sh_output: String::new(),
            executed: false,
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

    /// Showing current path like actual Shell did
    pub fn get_path(&self) -> String {
        let path = self.curr_path.to_string_lossy().into_owned();
        path
    }

    fn input_reset(&self) {
        self.sh_input.borrow_mut().reset();
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

    pub fn new(model: &str) -> App {
        App {
            input: Input::default(),
            input_mode: EditMode::Normal,
            messages: OllamaReq::new(model),
            shell_commands: VecDeque::new(),
            shell: DummyShell::default(),
        }
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal, client: Bclient) -> io::Result<()> {
        loop {
            terminal.draw(|f| self.ui(f))?;

            if let Event::Key(key) = event::read()? {
                match self.input_mode {
                    EditMode::Normal => match key.code {
                        KeyCode::Char('q') => {
                            return Ok(())
                        },
                        KeyCode::Char('a') => {
                            self.input_mode = EditMode::Input;
                        },
                        KeyCode::Char('s') => {
                            self.input_mode = EditMode::Shell;
                        },
                        _ => {}
                    },
                    EditMode::Input => match key.code {
                        KeyCode::Enter => {
                            self.messages.prompt(&self.input.value());
                            let res = client.send_ollama(&self.messages).await.unwrap();
                            self.recv_from(res);
                            self.input.reset();
                            let mut input_ref = self.shell.sh_input.borrow_mut();
                            let comm = self.shell_commands.front().unwrap().clone();
                            *input_ref = input_ref.clone().with_value(comm);
                            drop(input_ref);
                            self.input_mode = EditMode::Normal;  // return to normal mode to avoid sends empty msg
                        },
                        KeyCode::Esc => {
                            self.input_mode = EditMode::Normal;
                        },
                        _ => {
                            self.input.handle_event(&Event::Key(key));
                        }
                    },
                    EditMode::Shell => match key.code {
                        KeyCode::Enter => {
                            let mut input_ref = self.shell.sh_input.borrow_mut();
                            let comm = input_ref.value();
                            self.shell.executed_command = comm.to_string();
                            let out_msg = self.shell.shell.run_command(comm);
                            self.shell.sh_output = match out_msg.code {
                                Some(0) => { String::from_utf8(out_msg.stdout).unwrap() },
                                None => { "This command has no output".to_string() },
                                _ => { String::from_utf8(out_msg.stderr).unwrap() },
                            };
                            // println!("current output: {}", &self.shell.sh_output);
                            let _ = if self.shell_commands.is_empty() { None }
                                else { Some(self.shell_commands.pop_front().unwrap()) };
                            if self.shell_commands.is_empty() {
                                drop(input_ref);
                                self.shell.input_reset();  // borrow mut here
                            } else {
                                let command = self.shell_commands.front().unwrap().clone();
                                *input_ref = input_ref.clone().with_value(command);
                            }
                            self.input_mode = EditMode::Normal;
                        },
                        KeyCode::Esc => {
                            self.input_mode = EditMode::Normal;
                        }
                        _ => {
                            let mut input_ref = self.shell.sh_input.borrow_mut();
                            input_ref.handle_event(&Event::Key(key));
                        }
                    }
                }
            }
        }
    }

    fn ui(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(1),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(24),
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

        /// Asking AI block
        let width = chunks[0].width.max(3) - 1;  // 2 for boarders and 1 for cursor
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


        /// Shell interact block
        let path = self.shell.get_path();
        /*
        let sh_to_render = if self.shell_commands.is_empty() {
            let input_ref = self.shell.sh_input.borrow_mut();
            format!("{} > {}", path, input_ref.value())
        } else {
            let command = self.shell_commands.front().unwrap().clone();
            let mut input_ref = self.shell.sh_input.borrow_mut();
            *input_ref = input_ref.clone().with_value(command);
            drop(input_ref);
            format!("{} > {}", path, self.shell.sh_input.borrow().value())
        };
        */
        let input_ref_val = self.shell.sh_input.borrow();
        let sh_to_render = format!("{} > {}", path, input_ref_val.value());
        drop(input_ref_val);
        let sh_para = Paragraph::new(sh_to_render.clone())
            .style(match self.input_mode {
                EditMode::Normal => Style::default(),
                EditMode::Input => Style::default().fg(Color::Blue),
                EditMode::Shell => Style::default().fg(Color::Yellow),
            })
            .scroll((0, scroll as u16))
            .block(Block::default().borders(Borders::ALL).title("Shell"));
        frame.render_widget(sh_para, chunks[2]);

        /// Shell output block
        let binding = self.shell.sh_input.clone();
        let val_ref = binding.borrow();
        let sh_msg = format!("Command: {}, Output: {}", self.shell.executed_command, self.shell.sh_output);
        let sh_output = Paragraph::new(sh_msg)
            .style(match self.input_mode {
                EditMode::Normal => Style::default(),
                _ => Style::default().fg(Color::White),
            })
            .block(Block::default().borders(Borders::ALL).title("Output"));
        frame.render_widget(sh_output, chunks[3]);

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
                frame.set_cursor_position((
                    chunks[2].x
                        + (val_ref.visual_cursor().max(scroll + sh_to_render.len()) - scroll) as u16
                        + 1,
                    chunks[2].y + 1
                ));
            }
        }
    }

    /// Store received commands
    pub fn recv_from(&mut self, rece_vec: Vec<String>) {
        self.shell_commands = VecDeque::from(rece_vec);
    }
}
