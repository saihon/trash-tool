mod color;
mod file_type;
mod spec;
mod url_escape;

pub mod emptying;
pub mod error;
pub mod listing;
pub mod locations;
pub mod restoring;
pub mod trashing;

pub use color::apply_color_setting;
pub use emptying::handle_empty_trash;
pub use error::AppError;
pub use listing::handle_display_trash;
pub use restoring::handle_interactive_restore;
pub use trashing::handle_move_to_trash;
