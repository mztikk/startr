use rayon::iter::IntoParallelRefIterator;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Child,
};
use structopt::StructOpt;

#[derive(Deserialize, Serialize)]
enum CommandType {
    Command(String),
    Execution {
        command: String,
        working_directory: Option<String>,
        args: Option<Vec<String>>,
        #[serde(default = "bool::default")]
        spawn_only: bool,
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
    child: std::result::Result<Child, std::io::Error>,
    wait: bool,
}

fn run(command: &CommandType) -> ExecutionResult {
    match command {
        CommandType::Command(cmd) => {
            if cfg!(target_os = "windows") {
                let child = std::process::Command::new("cmd").arg("/C").arg(cmd).spawn();
                return ExecutionResult {
                    command: cmd.to_string(),
                    child,
                    wait: true,
                };
            } else {
                let child = std::process::Command::new("sh").arg("-c").arg(cmd).spawn();
                return ExecutionResult {
                    command: cmd.to_string(),
                    child,
                    wait: true,
                };
            }
        }
        CommandType::Execution {
            command,
            working_directory,
            args,
            spawn_only,
        } => {
            let cmd = format!("{} in {:?} with {:?}", command, working_directory, &args);
            let child = std::process::Command::new(command)
                .current_dir(
                    working_directory
                        .as_ref()
                        .map_or(std::env::current_dir().unwrap(), |d| {
                            Path::new(&d).to_path_buf()
                        }),
                )
                .args(args.as_ref().unwrap_or(&Vec::new()))
                .spawn();
            return ExecutionResult {
                command: cmd,
                child,
                wait: !spawn_only,
            };
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::from_args();

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
                let result = run(&cmd);
                println!(
                    "{}:\n{}",
                    result.command,
                    if result.wait {
                        String::from_utf8_lossy(&result.child?.wait_with_output()?.stdout)
                            .to_string()
                    } else {
                        "spawned".to_string()
                    }
                );
            }
            Command::Parallel(cmds) => {
                cmds.par_iter().for_each(|cmd| {
                    let result = run(&cmd);
                    println!(
                        "{}:\n{}",
                        result.command,
                        if result.wait {
                            String::from_utf8_lossy(
                                &result.child.unwrap().wait_with_output().unwrap().stdout,
                            )
                            .to_string()
                        } else {
                            "spawned".to_string()
                        }
                    );
                });
            }
        }
    }

    Ok(())
}
