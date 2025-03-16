use clap::{Subcommand, Parser, CommandFactory};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::env;
use std::path::Path;
use serde::de::Error;
use aurish::shared::Config;
use aurish::backend::{BKclient, OllamaReq, ClientInit};
use aurish::frontend::App_cli;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Set proxy (e.g., --set-proxy http://proxy.example.com:port)
    #[arg(long = "set-proxy")]
    set_proxy: Option<String>,

    /// Set ollama API (e.g., --set-ollama-api "http://localhost:11434/api/generate")
    #[arg(long = "set-ollama-api")]
    set_ollama_api: Option<String>,

    /// Set model (e.g., --set-model llama3:8b)
    #[arg(long = "set-model")]
    set_model: Option<String>,

    /// Subcommand to execute: show or dry-run or run
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show current configuration
    Show,
    /// Execute a dry run of the configuration
    // #[command(alias = "dry-run")]
    DryRun,
    /// Execute aurish-cli interactive version (lightweight compare to aurish)
    // #[command(alias = "run")]
    Run,
}

fn main() -> Result<(), Box<dyn std::error::Error>>{
    let args = Args::parse();
    let mut config = get_config().unwrap();

    if let Some(proxy) = args.set_proxy {
        config.set_proxy(proxy);
        write_to(config).unwrap();
        return Ok(());
    }
    if let Some(api) = args.set_ollama_api {
        config.set_ollama_api(api);
        write_to(config).unwrap();
        return Ok(());
    }
    if let Some(model) = args.set_model {
        config.set_model(model);
        write_to(config).unwrap();
        return Ok(());
    }

    if let Some(cmd) = args.command {
        match cmd {
            Commands::Show => {
                println!("Config: {:?}", config);
                return Ok(())
            },
            Commands::DryRun => {
                dry_run(config);
                return Ok(())
            },
            Commands::Run => {
                run_app_cli(config).unwrap();
                return Ok(())
            }
        }
    } else {
        Args::command().print_help().unwrap();
        println!();
    }

    Ok(())
}

pub fn get_config() -> Result<Config, Box<dyn std::error::Error>> {
    if let Ok(contents) = fs::read_to_string("config.json") {
        let config: Config = serde_json::from_str(&contents).unwrap();
        Ok(config)
    } else {
        let default_config = Config::default();
        let json_str = serde_json::to_string_pretty(&default_config).unwrap();
        let path = Path::new("./config.json");
        let mut file = File::create(path).unwrap();
        file.write_all(json_str.as_bytes())?;
        Ok(default_config)
    }
}

pub fn write_to(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let json_str = serde_json::to_string_pretty(&config)?;
    let path = Path::new("./config.json");
    let mut file = File::open(path)?;
    file.write_all(json_str.as_bytes())?;
    Ok(())
}

pub fn dry_run(config: Config) {
    let mut req = OllamaReq::new(&config.get_model());
    println!("Data to send: {:#?}", &req);
    req.prompt("How to show all files within current path? And then create a folder named test under current path.");
    if config.uses_proxy() {
        let client = BKclient::new_with_proxy(&config.get_ollama_api(), &config.get_proxy());
        let res = client.send_ollama(&req).unwrap();
        println!("ollama response: {:?}", res)
    } else {
        let client = BKclient::new(&config.get_ollama_api());
        let res = client.send_ollama(&req).unwrap();
        println!("ollama response: {:?}", res)
    }
}

pub fn run_app_cli(config: Config) -> Result<(), rustyline::error::ReadlineError> {
    if config.uses_proxy() {
        let client = BKclient::new_with_proxy(&config.get_ollama_api(), &config.get_proxy());
        let mut app = App_cli::new(&config.get_model());
        app.run(client)
    } else {
        let client = BKclient::new(&config.get_ollama_api());
        let mut app = App_cli::new(&config.get_model());
        app.run(client)
    }
}

