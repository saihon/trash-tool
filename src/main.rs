mod cli;
pub mod trash;

use cli::{parse_args, Commands};

use crate::trash::{
    apply_color_setting, find_all_trash_dirs, handle_display_trash, handle_empty_trash, handle_interactive_restore,
    handle_move_to_trash, AppError,
};

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1)
    }
    std::process::exit(0)
}

/// The primary function containing all application logic.
fn run() -> Result<(), AppError> {
    let args = parse_args()?;

    apply_color_setting(&args.color);

    if args.restore {
        if let Some(Commands::UI(skim_options)) = args.command {
            return handle_interactive_restore(skim_options);
        }
        return Ok(());
    }

    let trash_dirs = find_all_trash_dirs()?;

    if !args.files.is_empty() {
        return handle_move_to_trash(&trash_dirs, &args.files);
    }

    // Default action: if no files are given and no action is specified, display the trash contents.
    let is_default_action = !(args.long || args.display || args.empty || args.no_confirm);

    if args.long || args.display || is_default_action {
        handle_display_trash(&trash_dirs, args.long)?;
    }

    if args.empty || args.no_confirm {
        handle_empty_trash(&trash_dirs, args.no_confirm)?;
    }

    Ok(())
}
