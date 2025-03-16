//! Interactive shell for Rust
//!
//! Provides an IShell interface to run commands through.
//! These are the advantages:
//! - Each command returns an `std::process::Output` type with stdout and stderr captured (while also being logged)
//! - `cd` commands are remembered, despite each command running sequentially, each in a new true shell (i.e. `sh`)

#![warn(missing_docs)]

use std::env;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

/// A module for handling shell initialization errors.
///
/// This module defines the `ShellInitError` enum, which represents various errors
/// that can occur when attempting to initialize a shell. These errors primarily
/// relate to directory access, including issues with directory existence and permissions.
///
/// The `ShellInitError` enum provides a way to handle errors when constructing an
/// `IShell` instance with `IShell::from_path(...).


use crate::error::ShellInitError;

#[cfg(feature = "logging")]
use log::{error, info, warn};

/// Leech output from stdout/stderr while also storing the resulting output
macro_rules! leech_output {
    ($out:ident, $out_buf:ident, $log_method:ident) => {
        thread::spawn({
            let output_buffer_clone = Arc::clone($out_buf);
            move || {
                if let Some(output) = $out {
                    let reader = BufReader::new(output);
                    for line in reader.lines() {
                        if let Ok(line) = line {
                            #[cfg(feature = "logging")]
                            $log_method!("{}", line);
                            match output_buffer_clone.lock() {
                                Err(_err) => {
                                    #[cfg(feature = "logging")]
                                    error!("Failed to lock {} buffer! {}", stringify!($out), _err);
                                    return;
                                }
                                Ok(mut vec) => {
                                    vec.push(line);
                                }
                            }
                        }
                    }
                }
            }
        })
    };
}

/// Representation of the output of a command executed in an IShell.
///
/// The `ShellOutput` struct holds the results of a command that was run through a shell,
/// including the exit code, standard output, and standard error output.
pub struct ShellOutput {
    /// An optional exit code returned by the command.
    /// - If the command executed successfully, this will typically be `0`.
    /// - If the command failed or was terminated, this will contain a non-zero value.
    /// - If the command did not return an exit code, this will be `None`.
    pub code: Option<i32>,

    /// A vector of bytes containing the standard output produced by the command.
    /// - This field captures any output that the command printed to the standard output stream (if any).
    pub stdout: Vec<u8>,

    /// A vector of bytes containing the standard error output produced by the command.
    /// - This field captures any error messages or diagnostics that the command printed to the standard error stream.
    pub stderr: Vec<u8>,
}

impl ShellOutput {
    /// Check if output indicates a command was successful
    ///
    /// The check is done by comparing to 0.
    /// If no output is found, returns false
    pub fn is_success(&self) -> bool {
        self.code.unwrap_or(1) == 0
    }
}

/// A shell interface with memory
pub struct IShell {
    initial_dir: PathBuf,
    current_dir: Arc<Mutex<PathBuf>>,
    shell_type: ShellType,
}

#[derive(Debug)]
pub enum ShellType {
    PowerShell,
    Cmd,
    Bash,
    Fish,
    Zsh,
    Ksh,
    Unknown,
}

fn which_shell() -> ShellType {
    /// Detect which shell AI interact with.
    /// On windows, the default shell this function returned is PowerShell.
    if cfg!(target_os = "windows") {
        match env::var("PSModulePath") {
            Ok(_p) => return ShellType::PowerShell,
            Err(_e) => {
                match env::var("COMSPEC") {
                    Ok(_c) => return ShellType::Cmd,
                    Err(_e) => panic!("Shell Not found!"),
                }
            },
        }
    } else {
        match env::var("SHELL") {
            Ok(shell) => {
                let shell_lower = shell.to_lowercase();
                if shell_lower.contains("bash") {
                    return ShellType::Bash;
                } else if shell_lower.contains("zsh") {
                    return ShellType::Zsh;
                } else if shell_lower.contains("fish") {
                    return ShellType::Fish;
                } else if shell_lower.contains("ksh") {
                    return ShellType::Ksh;
                } else {
                    return ShellType::Unknown
                }
            },
            Err(_e) => panic!("Shell Not found!"),
        }
    }
}

impl Default for IShell {
    fn default() -> Self {
        Self::new()
    }
}

impl IShell {
    /// Constructs a new IShell with internal shell's
    /// directory set to the value of `std::env::current_dir()`.
    ///
    /// # Panics
    ///
    /// This function will panic due to `std::env::current_dir()` if any of the following is true:
    /// - Current directory (from where your program is ran) does not exist
    /// - There are insufficient permissions to access the current directory (from where your program is ran)
    /// - Directory (from where your program is ran) contains invalid UTF-8
    pub fn new() -> Self {
        let current_dir = env::current_dir().expect(
            "Failed to get current directory; it may not exist or you may not have permissions",
        );

        IShell {
            initial_dir: current_dir.clone(),
            current_dir: Arc::new(Mutex::new(current_dir)),
            shell_type: which_shell()
        }
    }

    /// Constructs a new IShell with internal shell's directory
    /// set to the value of
    ///
    /// <current_dir> / `initial_dir`
    ///
    /// if it exists.
    /// Otherwise, initial_dir is treated as a full path
    pub fn from_path(initial_dir: impl AsRef<Path>) -> Result<Self, ShellInitError> {
        let initial_dir = initial_dir.as_ref();

        let current_dir = env::current_dir().expect(
            "Failed to get current directory; it may not exist or you may not have permissions.",
        );

        match Self::determine_new_directory(&current_dir, initial_dir) {
            Some(new_dir) => Ok(IShell {
                initial_dir: new_dir.clone(),
                current_dir: Arc::new(Mutex::new(new_dir)),
                shell_type: which_shell(),
            }),
            None => Err(ShellInitError::DirectoryError(format!(
                "Couldn't open shell at either of {:#?} or {:#?}",
                initial_dir,
                current_dir.join(initial_dir)
            ))),
        }
    }

    /// Runs a command through IShell within its `current_dir`.
    ///
    /// Any `cd` command will not be _actually_ ran. Instead, inner directory of IShell (`current_dir`) will change
    /// accordingly. If `cd` is aliased to something else, (i.e. `changedir`), and you use this alias instead of `cd`,
    /// then IShell won't understand that you wanted it to change directory.
    pub fn run_command(&self, command: &str) -> ShellOutput {
        #[cfg(feature = "logging")]
        info!("Running: `{}`", command);

        if let Some(stripped_command) = command.strip_prefix("cd") {
            let new_dir = stripped_command.trim();
            let mut current_dir = self.current_dir.lock().unwrap();

            match Self::determine_new_directory(&*current_dir, new_dir) {
                Some(new_dir) => {
                    *current_dir = new_dir;
                    return self.create_output(Some(0), Vec::new(), Vec::new());
                }
                None => {
                    #[cfg(feature = "logging")]
                    {
                        error!("Failed to change directory to: {}", new_dir);
                        error!("Current directory: '{}'", current_dir.display());
                    }
                    return self.create_output(
                        Some(1),
                        Vec::new(),
                        Vec::from("Specified directory does not exist!"),
                    );
                }
            }
        }

        let child_process = self.spawn_process(command);
        match child_process {
            Ok(mut process) => {
                let (stdout_buffer, stderr_buffer) = (
                    Arc::new(Mutex::new(Vec::new())),
                    Arc::new(Mutex::new(Vec::new())),
                );

                let (stdout_handle, stderr_handle) = self.spawn_output_threads(
                    process.stdout.take(),
                    process.stderr.take(),
                    &stdout_buffer,
                    &stderr_buffer,
                );

                let status = process.wait().unwrap_or_else(|_err| {
                    #[cfg(feature = "logging")]
                    error!("Failed to wait for process: {}", _err);
                    ExitStatus::default()
                });

                if let Err(_err) = stdout_handle.join() {
                    #[cfg(feature = "logging")]
                    error!("Failed to join stdout thread: {:?}", _err);
                }
                if let Err(_err) = stderr_handle.join() {
                    #[cfg(feature = "logging")]
                    error!("Failed to join stderr thread: {:?}", _err);
                }

                let stdout = self.collect_output(&stdout_buffer);
                let stderr = self.collect_output(&stderr_buffer);

                ShellOutput {
                    code: status.code(),
                    stdout,
                    stderr,
                }
            }
            Err(e) => {
                #[cfg(feature = "logging")]
                error!("Couldn't spawn child process! {}", e);

                self.create_output(Some(-1), Vec::new(), Vec::from(format!("Error: {}", e)))
            }
        }
    }

    /// Forget current directory and go back to the directory initially specified.
    pub fn forget_current_directory(&self) {
        let mut current_dir = self.current_dir.lock().unwrap();
        *current_dir = self.initial_dir.clone();
    }

    fn create_output(&self, code: Option<i32>, stdout: Vec<u8>, stderr: Vec<u8>) -> ShellOutput {
        ShellOutput {
            code,
            stdout,
            stderr,
        }
    }

    fn spawn_process(&self, command: &str) -> std::io::Result<std::process::Child> {
        let current_dir = self.current_dir.lock().unwrap().clone();
        let (shell, arg) = match self.shell_type {
            ShellType::PowerShell => {
                ("powershell", "-Command")
            },
            ShellType::Cmd => {
                ("cmd", "/C")
            },
            ShellType::Bash => {
                ("sh", "-c")
            },
            ShellType::Fish => {
                ("fish", "-c")
            },
            ShellType::Zsh => {
                ("zsh", "-c")
            },
            ShellType::Ksh => {
                ("ksh", "-c")
            }
            ShellType::Unknown => {
                panic!("Unknown Shell type")
            }
        };

        Command::new(shell)
            .arg(arg)
            .arg(command)
            .current_dir(current_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }

    fn spawn_output_threads(
        &self,
        stdout: Option<std::process::ChildStdout>,
        stderr: Option<std::process::ChildStderr>,
        stdout_buffer: &Arc<Mutex<Vec<String>>>,
        stderr_buffer: &Arc<Mutex<Vec<String>>>,
    ) -> (thread::JoinHandle<()>, thread::JoinHandle<()>) {
        let stdout_handle = leech_output!(stdout, stdout_buffer, info);
        let stderr_handle = leech_output!(stderr, stderr_buffer, warn);

        (stdout_handle, stderr_handle)
    }

    fn collect_output(&self, buffer: &Arc<Mutex<Vec<String>>>) -> Vec<u8> {
        match buffer.lock() {
            Ok(buffer) => buffer.join("\n").into_bytes(),
            Err(_err) => {
                #[cfg(feature = "logging")]
                error!("Couldn't lock buffer! {}", _err);
                // Need to return SOMETHING here.
                Vec::new()
            }
        }
    }

    /// Method to quickly check if given path is a valid directory
    fn is_valid_directory(path: &Path) -> bool {
        path.exists() && path.is_dir()
    }

    /// Method to determine the new directory
    /// Checks if `current_dir`/`new_dir` is a valid dir (and returns it if it is),
    /// if it isn't - checks if `new_dir` is a valid dir (and returns it if it is);
    /// if it isn't - returns None
    fn determine_new_directory<U: AsRef<Path>, T: AsRef<Path>>(
        current_dir: U,
        new_dir: T,
    ) -> Option<PathBuf> {
        let new_dir = new_dir.as_ref();
        let current_dir = current_dir.as_ref();

        // Perhaps the `new_dir` is relative to `current_dir`?
        let wanted_dir = current_dir.join(new_dir);
        if Self::is_valid_directory(&wanted_dir) {
            return Some(wanted_dir.to_path_buf());
        }

        // Maybe `new_dir` wasn't relative?
        if let Some(sanitized_dir) = Self::sanitize_path(new_dir) {
            if Self::is_valid_directory(&sanitized_dir) {
                return Some(sanitized_dir);
            } else {
                #[cfg(feature = "logging")]
                warn!(
                    "Neither the combined path {:#?} nor the sanitized path {:#?} is a valid directory.",
                    wanted_dir, sanitized_dir
                );
            }
        }

        // I guess `new_dir` doesn't exist...
        None
    }

    /// Expand tilde
    /// Inspired by https://github.com/splurf/simple-expand-tilde/blob/master/src/lib.rs
    fn sanitize_path(path: impl AsRef<Path>) -> Option<PathBuf> {
        let resolved_path = path.as_ref();

        if !resolved_path.starts_with("~") {
            return Some(resolved_path.to_path_buf());
        }
        if resolved_path == Path::new("~") {
            return dirs::home_dir();
        }

        dirs::home_dir().map(|mut home_dir| {
            if home_dir == Path::new("/") {
                // For when running as root
                resolved_path.strip_prefix("~").unwrap().to_path_buf()
            } else {
                home_dir.push(resolved_path.strip_prefix("~/").unwrap());
                home_dir
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn true_command() {
        let shell = IShell::new();

        let result = shell.run_command("true");
        assert!(result.is_success());
    }

    #[test]
    fn false_command() {
        let shell = IShell::new();

        let result = shell.run_command("false");
        assert!(!result.is_success());
    }

    #[test]
    fn echo_command() {
        // Checking stdout capture
        let shell = IShell::new();

        let result = shell.run_command("echo \"Hello, World!\"");
        let stdout_res = String::from_utf8(result.stdout).expect("Stdout contained invalid UTF-8!");
        assert_eq!(stdout_res, "Hello, World!");
    }

    #[test]
    fn dir_memory() {
        // Check for whether CD is remembered

        let shell = IShell::new();

        let unique_dir_1 = format!("test_{}", rand::random::<u32>());
        let unique_dir_2 = format!("test2_{}", rand::random::<u32>());

        shell.run_command(&format!("mkdir {}", unique_dir_1));
        shell.run_command(&format!("cd {}", unique_dir_1));
        shell.run_command(&format!("mkdir {}", unique_dir_2));

        let result = shell.run_command("ls");
        let stdout_res = String::from_utf8(result.stdout).expect("Stdout contained invalid UTF-8!");
        assert_eq!(stdout_res.trim(), unique_dir_2);

        shell.run_command("cd ..");
        shell.run_command(&format!("rm -r {}", unique_dir_1));
    }

    #[test]
    fn forget_current_dir() {
        let shell = IShell::new();

        let result = shell.run_command("echo $PWD");
        let pwd = String::from_utf8(result.stdout).expect("Stdout contained invalid UTF-8!");

        let unique_dir = format!("test_{}", rand::random::<u32>());

        shell.run_command(&format!("mkdir {}", unique_dir));
        shell.run_command(&format!("cd {}", unique_dir));
        shell.forget_current_directory();

        let result = shell.run_command("echo $PWD");
        let forgotten_pwd =
            String::from_utf8(result.stdout).expect("Stdout contained invalid UTF-8!");

        assert_eq!(pwd, forgotten_pwd);

        shell.run_command(&format!("rm -r {}", unique_dir));
    }

    #[test]
    fn dir_doesnt_exist() {
        let shell = IShell::new();

        let current_dir = shell.current_dir.lock().unwrap().clone();
        let res = shell.run_command("cd directory_that_doesnt_exist");
        let next_dir = shell.current_dir.lock().unwrap().clone();

        assert!(!res.is_success());
        assert_eq!(current_dir, next_dir);
    }

    #[test]
    fn relative_construct() {
        let main_shell = IShell::new();
        main_shell.run_command("cd target");
        let main_result = main_shell.run_command("ls");
        assert!(main_result.is_success());

        let target_shell = IShell::from_path("target").unwrap();
        let target_result = target_shell.run_command("ls");

        let target_result =
            String::from_utf8(target_result.stdout).expect("Stdout contained invalid UTF-8!");
        let main_result =
            String::from_utf8(main_result.stdout).expect("Stdout contained invalid UTF-8!");

        assert_eq!(target_result, main_result);
    }

    #[test]
    fn tilda_init() {
        let desktop_shell = IShell::from_path("~").unwrap();
        let shell = IShell::new();

        shell.run_command("cd ~");
        let res = shell.run_command("ls");
        let desktop_res = desktop_shell.run_command("ls");

        let res = String::from_utf8(res.stdout).expect("Stdout contained invalid UTF-8!");
        let desktop_res =
            String::from_utf8(desktop_res.stdout).expect("Stdout contained invalid UTF-8!");

        assert_eq!(res, desktop_res);
    }
}
