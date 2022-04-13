use clap::Parser;
use core::fmt;
use rayon::iter::IntoParallelRefIterator;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Child,
};

#[derive(Deserialize, Serialize)]
enum CommandType {
    Command(String),
    Execution {
        command: String,
        working_directory: Option<String>,
        #[serde(default = "Vec::new")]
        args: Vec<String>,
        #[serde(default = "bool::default")]
        spawn_only: bool,
    },
}

impl fmt::Display for CommandType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandType::Command(cmd) => write!(f, "{}", cmd),
            CommandType::Execution {
                command,
                working_directory,
                args,
                spawn_only: _,
            } => write!(f, "{} in {:?} with {:?}", command, working_directory, args),
        }
    }
}

#[derive(Deserialize, Serialize)]
enum Command {
    Single(CommandType),
    Parallel(Vec<CommandType>),
}

#[derive(Parser, Debug)]
struct Cli {
    #[clap(parse(from_os_str))]
    config: Option<PathBuf>,
}

struct ExecutionResult {
    child: std::result::Result<Child, std::io::Error>,
    wait: bool,
}

fn shell_command() -> std::process::Command {
    if cfg!(windows) {
        let mut cmd = std::process::Command::new("cmd");
        cmd.arg("/C");
        cmd
    } else {
        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c");
        cmd
    }
}

fn run(command: &CommandType) -> ExecutionResult {
    match command {
        CommandType::Command(cmd) => {
            return ExecutionResult {
                child: shell_command().arg(cmd).spawn(),
                wait: true,
            };
        }
        CommandType::Execution {
            command,
            working_directory,
            args,
            spawn_only,
        } => {
            let child = std::process::Command::new(command)
                .current_dir(
                    working_directory
                        .as_ref()
                        .map_or(std::env::current_dir().unwrap(), |d| {
                            Path::new(d).to_path_buf()
                        }),
                )
                .args(args)
                .spawn();
            ExecutionResult {
                child,
                wait: !spawn_only,
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();

    let config_file = match args.config {
        Some(path) => path,
        None => std::env::current_exe()?.with_extension("yaml"),
    };

    println!("{}", config_file.to_string_lossy());

    let config = std::fs::read_to_string(config_file)?;

    let commands: Vec<Command> = serde_yaml::from_str(&config)?;

    for command in commands {
        match command {
            Command::Single(cmd) => {
                println!("{}", cmd);
                let result = run(&cmd);
                println!(
                    "{}",
                    if result.wait {
                        String::from_utf8_lossy(&result.child?.wait_with_output()?.stdout)
                            .to_string()
                    } else {
                        format!("spawned {}", cmd)
                    }
                );
            }
            Command::Parallel(cmds) => {
                cmds.par_iter().for_each(|cmd| {
                    println!("{}", cmd);
                    let result = run(cmd);
                    println!(
                        "{}",
                        if result.wait {
                            String::from_utf8_lossy(
                                &result.child.unwrap().wait_with_output().unwrap().stdout,
                            )
                            .to_string()
                        } else {
                            format!("spawned {}", cmd)
                        }
                    );
                });
            }
        }
    }

    Ok(())
}
