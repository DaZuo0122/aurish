use std::fs;
use std::fs::File;
use std::io::Write;
use std::env;
use std::path::Path;
use aurish::shared::Config;
use aurish::backend::{Bclient, OllamaReq};
use tokio;

#[tokio::main]
async fn main() {
    let commands: Vec<&str> = Vec::from([
        "--set-proxy",
        "--set-ollama-api",
        "--set-model",
        "show",
        "dry-run",
    ]);
    let help_msg: String = format!("usage: aurish-cli [--flags] value \n
or aurish-cli [commands] \n
example: aurish-cli --set-model llama3:8b \n
aurish-cli dry-run \n
available commands and flags: {:?}", &commands);
    let args: Vec<String> = env::args().collect();
    let mut iter = args.into_iter().skip(1);
    let mut config = get_config().unwrap();
    let arg = iter.next().unwrap();
    match arg.as_str() {
        "--set-proxy" => {
            if let Some(value) = iter.next() {
                if !commands.contains(&value.as_str()) {
                    config.set_proxy(value);
                    write_to(config).unwrap();
                } else { println!("{}", &help_msg); }
            } else { println!("{}", &help_msg); }
        },
        "--set-ollama-api" => {
            if let Some(value) = iter.next() {
                if !commands.contains(&value.as_str()) {
                    config.set_ollama_api(value);
                    write_to(config).unwrap();
                } else { println!("{}", &help_msg); }
            } else { println!("{}", &help_msg); }
        },
        "--set-model" => {
            if let Some(value) = iter.next() {
                if !commands.contains(&value.as_str()) {
                    config.set_model(value);
                    write_to(config).unwrap();
                } else {
                    println!("{}", &help_msg);
                }
            } else {
                println!("{}", &help_msg);
            }
        },
        "show" => {
            if let Some(_value) = iter.next() {
                println!("{}", &help_msg);
            } else {
                println!("{:?}", config);
            }
        },
        "dry-run" => {
            if let Some(_value) = iter.next() {
                println!("{}", &help_msg);
            } else { dry_run(config).await; }
        }
        _ => {
            println!("{}", &help_msg);
        }
    }
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

pub async fn dry_run(config: Config) {
    let mut req = OllamaReq::new(&config.get_model());
    println!("Data to send: {:#?}", &req);
    req.prompt("How to change current path to D://foo/bar? And then create a folder named test under current path.");
    if config.uses_proxy() {
        let client = Bclient::new_with_proxy(&config.get_ollama_api(), &config.get_proxy());
        let res = client.send_ollama(&req).await.unwrap();
        println!("ollama response: {:?}", res)
    } else {
        let client = Bclient::new(&config.get_ollama_api());
        let res = client.send_ollama(&req).await.unwrap();
        println!("ollama response: {:?}", res)
    }
}

