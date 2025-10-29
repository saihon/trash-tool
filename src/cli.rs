use std::env;

use clap::ArgAction;
use clap::Parser;
use skim::SkimOptions;

use crate::trash::error::AppError;

/// A command-line trash can utility that adheres to the FreeDesktop.org specification.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Files or directories to move to the trash
    pub files: Vec<String>,

    /// When to use colors.
    #[arg(long = "color", value_name = "WHEN", default_value = "auto", value_parser = ["auto", "always", "never"])]
    pub color: String,

    /// Display the contents of the trash directories.
    #[arg(short = 'd', long, action = ArgAction::SetTrue)]
    pub display: bool,

    /// List trash contents in a long format.
    #[arg(short = 'l', long, action = ArgAction::SetTrue)]
    pub long: bool,

    /// Permanently delete all contents of the trash directories.
    #[arg(short = 'e', long, action = ArgAction::SetTrue)]
    pub empty: bool,

    /// Empty the trash without prompting for confirmation.
    #[arg(short = 'y', long, action = ArgAction::SetTrue)]
    pub no_confirm: bool,

    /// Interactively restore items from the trash.
    #[arg(short = 'r', long, action = ArgAction::SetTrue)]
    pub restore: bool,

    /// Optional subcommand for advanced configuration, e.g., 'skim'.
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Parser)]
pub enum Commands {
    /// Configure the fuzzy finder options for restoring.
    #[command(name = "ui")]
    UI(SkimOptions),
}

const TRASH_TOOL_OPTIONS: &str = "TRASH_TOOL_OPTIONS";

fn parse_subcommand_with_env(cli_args: Vec<String>) -> Result<Option<Commands>, AppError> {
    let mut skim_args = vec![cli_args[0].clone()];

    skim_args.extend(shlex::split(&env::var(TRASH_TOOL_OPTIONS).unwrap_or_default()).unwrap_or_default());

    let skim_pos = cli_args.iter().position(|arg| arg == "skim");
    if skim_pos.is_some() {
        skim_args.extend_from_slice(&cli_args[skim_pos.unwrap() + 1..]);
    }

    let skim_options = SkimOptions::try_parse_from(skim_args).map_err(|e| AppError::Message(e.to_string()))?;

    Ok(Some(Commands::UI(skim_options)))
}

pub fn parse_args() -> Result<Args, AppError> {
    // Parse of all CLI arguments. A reason for this is to let `clap` handle subcommand help flags (e.g., `skim --help`) correctly.
    let mut args = Args::parse();

    if args.restore {
        args.command = parse_subcommand_with_env(env::args().collect())?;
    }

    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_cli_constants() {
        assert_eq!(TRASH_TOOL_OPTIONS, "TRASH_TOOL_OPTIONS");
    }

    #[test]
    #[serial]
    fn test_parse_subcommand_with_env_no_args_no_env() {
        env::remove_var(TRASH_TOOL_OPTIONS);
        let cli_args = vec!["trash-tool".to_string(), "-r".to_string()];

        let result = parse_subcommand_with_env(cli_args);

        assert!(result.is_ok());
        let command = result.unwrap();
        assert!(command.is_some());
        if let Some(Commands::UI(options)) = command {
            assert!(!options.multi);
            assert_eq!(options.height, "100%");
        } else {
            panic!("Expected Commands::UI");
        }
    }

    #[test]
    #[serial]
    fn test_parse_subcommand_with_env_only_env() {
        env::set_var(TRASH_TOOL_OPTIONS, "--multi --height 50%");
        let cli_args = vec!["trash-tool".to_string(), "-r".to_string()];

        let result = parse_subcommand_with_env(cli_args);

        assert!(result.is_ok());
        if let Some(Commands::UI(options)) = result.unwrap() {
            assert!(options.multi);
            assert_eq!(options.height, "50%");
        } else {
            panic!("Expected Commands::UI");
        }

        env::remove_var(TRASH_TOOL_OPTIONS);
    }

    #[test]
    #[serial]
    fn test_parse_subcommand_with_cli_only_cli() {
        env::remove_var(TRASH_TOOL_OPTIONS);
        let cli_args = vec![
            "trash-tool".to_string(),
            "-r".to_string(),
            "skim".to_string(),
            "--multi".to_string(),
            "--height".to_string(),
            "50%".to_string(),
        ];

        let result = parse_subcommand_with_env(cli_args);

        assert!(result.is_ok());
        if let Some(Commands::UI(options)) = result.unwrap() {
            assert!(options.multi);
            assert_eq!(options.height, "50%");
        } else {
            panic!("Expected Commands::UI");
        }

        env::remove_var(TRASH_TOOL_OPTIONS);
    }

    #[test]
    #[serial]
    fn test_parse_subcommand_with_env_cli_overrides_env() {
        env::set_var(TRASH_TOOL_OPTIONS, "--multi --height 50%");
        let cli_args = vec![
            "trash-tool".to_string(),
            "-r".to_string(),
            "skim".to_string(),
            "--height".to_string(),
            "80%".to_string(),
        ];

        let result = parse_subcommand_with_env(cli_args).unwrap().unwrap();

        let Commands::UI(options) = result;
        assert!(options.multi, "Should inherit --multi from env");
        assert_eq!(options.height, "80%", "Should use --height from CLI");

        env::remove_var(TRASH_TOOL_OPTIONS);
    }
}
