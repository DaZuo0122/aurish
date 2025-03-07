# Aurish
An AI integrated Shell, where you can review and modify AI given commands and then execute them seamlessly without annoying copy&paste. Everything is in your control with Aurish, it runs completely offline, and all the data are stored locally.

> [!NOTE]
> For privacy and safety concerns, Aurish only supports OLLAMA models for now.

> [!CAUTION]
> This project is under active development, and may encounter unexpected bugs when using.

## Known issues
 - Shell output cannot be displayed correctly.


## How to use
0. Make sure `Ollama` server is running  

1. Use `aurish-cli` to set the configuration, including model name, Ollama api endpoint and proxy.  
The default setting is
```json
{
	"ollama_api": "http://localhost:11434/api/generate",
	"model": "llama3:latest",
	"proxy": "",
}
```
Please note that the endpoint should be `/api/generate`.  

2. Use `aurish-cli dry-run` to test accessibility of Ollama server.  
You will see something like this:

```
Data to send: OllamaReq {
    model: "llama3:latest",
    prompt: "",
    stream: false,
    format: Object {
        "properties": Object {
            "commands": Object {
                "type": String("array"),
            },
        },
        "required": Array [
            String("commands"),
        ],
        "type": String("object"),
    },
    system: "You are PowerShell expert, your task is give PowerShell commands that meets user requirements. Your answer should only contains commands. Respond using JSON.",
}
ollama response: ["dir", "md test"]
```


4. Once everything set, type `aurish` to run


## Install
Download the pre-build binary at [release](https://github.com/DaZuo0122/aurish/releases).  

## Build 
0. Make sure you have Rust 2021 edition installed. 

1. Clone this repo

3. `cd aurish` and `cargo build --release`
