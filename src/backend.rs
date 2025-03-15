use reqwest::{Client, Proxy};
use reqwest::blocking::Client as BlockingClinet;
use serde::{Deserialize, Serialize, Deserializer};
use serde_json::{Result, Value, json};
use std::error::Error;
use std::string::ToString;
use std::collections::HashMap;
use std::env;


// pub const OLLAMA_GEN_API: String = String::from("http://localhost:11434/api/generate");

#[derive(Debug, Serialize)]
pub struct OllamaReq {
    model: String,
    prompt: String,
    stream: bool,
    format: Value,
    system: String,
}

#[derive(Debug, Deserialize)]
pub struct OllamaRes {
    model: String,
    created_at: String,
    response: String,
    done: bool,
    done_reason: String,
    context: Vec<u64>,
    total_duration: u64,
    load_duration: u64,
    prompt_eval_count: u64,
    prompt_eval_duration: u64,
    eval_count: u64,
    eval_duration: u64,
}

#[derive(Debug, Deserialize)]
pub struct Command {
    commands: Vec<String>
}

pub struct Bclient {
    client: Client,
    target: String,
}

pub struct BKclient {
    client: BlockingClinet,
    target: String,
}

impl OllamaReq {
    pub fn new(model: &str) -> OllamaReq {
        let shell_type = which_shell();
        OllamaReq {
            model: model.to_string(),
            prompt: String::new(),
            stream: false,
            format: json!(
                {
                    "type": "object",
                    "properties": {
                    "commands": {
                        "type": "array"
                    },
                },
                    "required": ["commands"]
                }
            ),
            system: format!("You are {} expert, your task is give {} commands that meets user requirements. Your answer should only contains commands. Respond using JSON.", &shell_type, &shell_type),
        }
    }

    pub fn prompt(&mut self, prompt: &str) {
        self.prompt = prompt.to_string();
    }

    pub fn set_model(&mut self, model: &str) {
        self.model = model.to_string();
    }

}

fn which_shell() -> String {
    /// Detect which shell AI interact with.
    /// On windows, the default shell this function returned is PowerShell.
    if cfg!(target_os = "windows") {
        match env::var("PSModulePath") {
            Ok(_p) => return "PowerShell".to_string(),
            Err(_e) => {
                match env::var("COMSPEC") {
                    Ok(_c) => return "Cmd".to_string(),
                    Err(_e) => panic!("Shell Not found!"),
                }
            },
        }
    } else {
        match env::var("SHELL") {
            Ok(shell) => {
                let shell_lower = shell.to_lowercase();
                if shell_lower.contains("bash") {
                    return "Bash".to_string();
                } else if shell_lower.contains("zsh") {
                    return "Zsh".to_string();
                } else if shell_lower.contains("fish") {
                    return "Fish".to_string();
                } else if shell_lower.contains("ksh") {
                    return "Ksh".to_string();
                } else {
                    panic!("Shell Not supported")
                }
            },
            Err(_e) => panic!("Shell Not found!"),
        }
    }
}

pub trait ClientInit {
    fn new(target: &str) -> Self;
    fn new_with_proxy(target: &str, proxy: &str) -> Self;
}

impl Default for Bclient {
    fn default() -> Self {
        Bclient {
            client: Client::new(),
            target: "http://localhost:11434/api/generate".to_string(),
        }
    }
}

impl Default for BKclient {
    fn default() -> Self {
        BKclient {
            client: BlockingClinet::new(),
            target: "http://localhost:11434/api/generate".to_string(),
        }
    }
}

impl ClientInit for Bclient {
    fn new(target: &str) -> Self {
        Bclient {
            client: Client::new(),
            target: target.to_string(),
        }
    }

    fn new_with_proxy(target: &str, proxy: &str) -> Self {
        Bclient {
            client: Client::builder()
                .proxy(Proxy::http(proxy).unwrap()).build().unwrap(),
            target: target.to_string(),
        }
    }
}

impl ClientInit for BKclient {
    fn new(target: &str) -> Self {
        BKclient {
            client: BlockingClinet::new(),
            target: target.to_string(),
        }
    }

    fn new_with_proxy(target: &str, proxy: &str) -> Self {
        BKclient {
            client: BlockingClinet::builder()
                .proxy(Proxy::http(proxy).unwrap()).build().unwrap(),
            target: target.to_string(),
        }
    }
}

impl Bclient {
    pub async fn send_ollama(&self, data: &OllamaReq) -> Result<Vec<String>> {
        // println!("Request body: {:#?}", &data);
        let res = self.client.post(&self.target)
            .json(data)
            .send()
            .await.unwrap();
        // println!("Raw response: {:#?}", &res);
        let res_body = res.text().await.unwrap();
        // println!("Response body: {:#?}", &res_body);
        let ollama_res: OllamaRes = serde_json::from_str(&res_body).unwrap();
        // println!("Ollama response: {:#?}", &ollama_res);
        let inner_json: Command = serde_json::from_str(&ollama_res.response).unwrap();
        Ok(inner_json.commands)
    }
}

impl BKclient {
    pub fn send_ollama(&self, data: &OllamaReq) -> Result<Vec<String>> {
        let res = self.client.post(&self.target)
            .json(data)
            .send()
            .unwrap();
        let res_body = res.text().unwrap();
        let ollama_res: OllamaRes = serde_json::from_str(&res_body).unwrap();
        let inner__json: Command = serde_json::from_str(&ollama_res.response).unwrap();
        Ok(inner__json.commands)
    }
}
