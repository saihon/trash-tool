mod cli;
pub mod trash;

use cli::{parse_args, Commands};

use crate::trash::{
    apply_color_setting, handle_display_trash, handle_empty_trash, handle_interactive_restore, handle_move_to_trash,
    AppError,
};

fn main() {
    if let Err(e) = run() {
        match e {
            AppError::Ignorable => {}
            _ => {
                eprintln!("Error: {}", e);
            }
        }
        std::process::exit(1)
    }
    std::process::exit(0)
}

/// The primary function containing all application logic.
fn run() -> Result<(), AppError> {
    let args = parse_args()?;

    apply_color_setting(&args.color);

    match true {
        _ if !args.files.is_empty() => {
            handle_move_to_trash(&args.files)?;
        }
        _ if args.restore => {
            if let Some(Commands::UI(skim_options)) = args.command {
                handle_interactive_restore(skim_options)?;
            }
        }
        _ if args.empty || args.no_confirm => {
            handle_empty_trash(args.no_confirm, args.display, args.long)?;
        }
        _ => {
            handle_display_trash(args.long)?;
        }
    }

    Ok(())
}
