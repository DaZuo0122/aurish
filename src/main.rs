use aurish::{shared::{App, Config}, backend::{OllamaReq, Bclient, ClientInit}};
use tokio;
use std::{fs, io};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use serde::de::Error;

#[tokio::main]
async fn main() -> io::Result<()> {
    // setup terminal
    enable_raw_mode()?;
    // execute!(EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = ratatui::init();

    // create app from config file and run it
    let config = get_config().unwrap();
    let mut app = App::new(config.get_model());
    let client = if config.uses_proxy() {
        Bclient::new_with_proxy(config.get_ollama_api(), config.get_proxy())
    } else { Bclient::new(config.get_ollama_api()) };
    let res = app.run(&mut terminal, client);

    // disable_raw_mode()?;
    ratatui::restore();

    res.await  // Is the futures here ended program unexpectedly?
}

fn get_config() -> Result<Config, Box<dyn std::error::Error>> {
    if let Ok(contents) = fs::read_to_string("config.json") {
        let config: Config = serde_json::from_str(&contents).unwrap();
        Ok(config)
    } else {
        panic!("config.json not found. Please set it up with aurish-cli")
    }
}