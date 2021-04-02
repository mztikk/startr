use rayon::iter::IntoParallelRefIterator;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::io::Error;
use std::{
    env,
    path::{Path, PathBuf},
    process::Output,
};
use structopt::StructOpt;

#[derive(Debug)]
enum ProgError {
    NoFile,
    NotUtf8,
    Io(Error),
}

impl From<Error> for ProgError {
    fn from(err: Error) -> ProgError {
        ProgError::Io(err)
    }
}

#[derive(Deserialize, Serialize)]
enum CommandType {
    Command(String),
    Execution {
        command: String,
        working_directory: Option<String>,
        args: Option<Vec<String>>,
    },
}

#[derive(Deserialize, Serialize)]
enum Command {
    Single(CommandType),
    Parallel(Vec<CommandType>),
}

#[derive(StructOpt)]
struct Cli {
    #[structopt(parse(from_os_str))]
    config: Option<PathBuf>,
}

struct ExecutionResult {
    command: String,
    result: std::result::Result<Output, std::io::Error>,
}

fn run(command: &CommandType) -> ExecutionResult {
    match command {
        CommandType::Command(cmd) => {
            if cfg!(target_os = "windows") {
                return ExecutionResult {
                    command: cmd.to_string(),
                    result: std::process::Command::new("cmd")
                        .arg("/C")
                        .arg(cmd)
                        .output(),
                };
            } else {
                return ExecutionResult {
                    command: cmd.to_string(),
                    result: std::process::Command::new("sh").arg("-c").arg(cmd).output(),
                };
            }
        }
        CommandType::Execution {
            command,
            working_directory,
            args,
        } => {
            let cmd = format!("{} in {:?} with {:?}", command, working_directory, &args);
            return ExecutionResult {
                command: cmd,
                result: std::process::Command::new(command)
                    .current_dir(
                        working_directory
                            .as_ref()
                            .map_or(std::env::current_dir().unwrap(), |d| {
                                Path::new(&d).to_path_buf()
                            }),
                    )
                    .args(args.as_ref().unwrap_or(&Vec::new()))
                    .output(),
            };
        }
    }
}

fn binary() -> Result<String, ProgError> {
    let path = env::current_exe()?;
    let name = path.file_name().ok_or(ProgError::NoFile)?;
    let s_name = name.to_str().ok_or(ProgError::NotUtf8)?;
    Ok(s_name.to_owned())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::from_args();

    let config_file = match args.config {
        Some(path) => path,
        None => {
            let mut path = std::env::current_exe().unwrap();
            path.pop();
            path.push(format!("{}.yaml", binary().unwrap()));
            path
        }
    };

    println!("{}", config_file.to_string_lossy());

    let config = std::fs::read_to_string(config_file)?;

    let commands: Vec<Command> = serde_yaml::from_str(&config)?;

    for command in commands {
        match command {
            Command::Single(cmd) => {
                let result = run(&cmd);
                println!(
                    "{}:\n{}",
                    result.command,
                    String::from_utf8_lossy(&result.result?.stdout)
                );
            }
            Command::Parallel(cmds) => {
                cmds.par_iter().for_each(|cmd| {
                    let result = run(&cmd);
                    println!(
                        "{}:\n{}",
                        result.command,
                        String::from_utf8_lossy(&result.result.unwrap().stdout)
                    );
                });
            }
        }
    }

    Ok(())
}
