use std::env::current_dir;
use rustyline::{DefaultEditor, Result};
use rustyline::error::ReadlineError;
// use ishell::IShell;
use std::path::PathBuf;
use std::collections::VecDeque;
use crate::shared::EditMode;
use crate::backend::{OllamaReq, ClientInit, BKclient};
use crate::shell::IShell;


pub struct App_cli {
    shell: Shell_cli,
    cli: DefaultEditor,
    edit_mode: EditMode,
    message: OllamaReq,
    shell_commands: VecDeque<String>,
}

struct Shell_cli {
    shell: IShell,
    curr_path: PathBuf,
}

impl Default for Shell_cli {
    fn default() -> Self {
        Shell_cli {
            shell: IShell::new(),
            curr_path: current_dir().unwrap(),
        }
    }
}

impl Shell_cli {
    pub fn renew_path(&mut self) {
        self.curr_path = current_dir().unwrap();
    }

    /// Showing current path like actual Shell did
    pub fn get_path(&self) -> String {
        let path = self.curr_path.to_string_lossy().into_owned();
        path
    }
}

impl App_cli {
    pub fn new(model: &str) -> App_cli {
        App_cli {
            shell: Shell_cli::default(),
            cli: DefaultEditor::new().unwrap(),
            edit_mode: EditMode::Input,
            message: OllamaReq::new(model),
            shell_commands: VecDeque::new(),
        }
    }

    /// Using Blocking Client to reduce overhead
    pub fn run(&mut self, client: BKclient) -> Result<()> {
        loop {
            match self.edit_mode {
                EditMode::Input => {
                    let title = "Asking AI >> ";
                    let readline = self.cli.readline(title);
                    match readline {
                        Ok(line) => {
                            self.message.prompt(line.as_str());
                            println!("Generating...");
                            let res = client.send_ollama(&self.message).unwrap();
                            self.recv_from(res);
                            self.edit_mode = EditMode::Shell;
                        },
                        Err(ReadlineError::Interrupted) => {
                            println!("Keyboard Interrupted");
                            println!("Program Closing...");
                            break;
                        },
                        Err(ReadlineError::Eof) => {
                            println!("CTRL-D");
                            break;
                        },
                        Err(err) => {
                            println!("Error: {:?}", err);
                            break;
                        }
                    }
                },
                EditMode::Shell => {
                    if self.shell_commands.is_empty() {
                        println!("No pending commands, return to Input Mode");
                        self.edit_mode = EditMode::Input;
                    } else {
                        self.shell.renew_path();
                        let prompt = format!("{}>> ", self.shell.get_path());
                        let command = self.shell_commands.front().unwrap().as_str();
                        let readline = self.cli.readline_with_initial(prompt.as_str(), (command, ""));
                        match readline {
                            Ok(line) => {
                                // execute on-screen command
                                let sh_result = self.shell.shell.run_command(line.as_str());
                                let result: String = if sh_result.is_success() {
                                    String::from_utf8(sh_result.stdout).expect("Stdout contained invalid UTF-8!")
                                } else {
                                    String::from_utf8(sh_result.stderr).expect("Stdout contained invalid UTF-8!")
                                };
                                println!("Shell output: {}", result);
                                // delete executed command
                                let _ = self.shell_commands.pop_front();
                            },
                            Err(ReadlineError::Interrupted) => {
                                println!("Keyboard Interrupted");
                                println!("Program Closing...");
                                break;
                            },
                            Err(ReadlineError::Eof) => {
                                println!("CTRL-D");
                                break;
                            },
                            Err(err) => {
                                println!("Error: {:?}", err);
                                break;
                            }
                        }
                    }
                },
                _ => {
                    println!("Unknown Error, quitting...");
                    println!("Debug Info:\n  Ollama msg: {:?}  \n Pending Commands: {:?}", self.message, self.shell_commands);
                    break;
                }
            }
        }

        Ok(())
    }

    pub fn recv_from(&mut self, rece_vec: Vec<String>) {
        self.shell_commands = VecDeque::from(rece_vec);
    }
}
