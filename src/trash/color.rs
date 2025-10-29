use std::path::Path;

use colored::{control, ColoredString, Colorize};

use super::file_type::{get_file_type, FileType};

/// Applies the global color setting based on the user's choice from CLI arguments.
/// This function centralizes control over the `colored` crate's behavior.
pub fn apply_color_setting(color_choice: &str) {
    match color_choice {
        "always" => control::set_override(true),
        "never" => control::set_override(false),
        "auto" | _ => {
            // "auto" is the default behavior of the `colored` crate, which checks if the output is a TTY.
            // No override is needed in this case.
        }
    }
}

/// Colorizes a string representing a trash directory
pub fn colorize_trash_directory(name: &str) -> ColoredString {
    name.white()
}

/// Colorizes the path based on its file type.
pub fn colorize_path(filename: &str, path: &Path) -> ColoredString {
    let file_type = get_file_type(path);

    match file_type {
        FileType::Directory => filename.blue().bold(),
        FileType::Executable => filename.green().bold(),
        FileType::Archive => filename.red().bold(),
        FileType::Config => filename.yellow().bold(),
        FileType::Document => filename.normal(),
        FileType::Image => filename.magenta().bold(),
        FileType::Video => filename.purple().bold(),
        FileType::Music => filename.cyan().bold(),
        FileType::Other => filename.normal(),
    }
}

/// Formats and colorizes the file mode (permissions) string.
#[cfg(unix)]
pub fn format_mode(mode: u32, is_dir: bool) -> String {
    let dir = if is_dir { "d".blue() } else { "-".dimmed() };

    let r = "r".yellow();
    let w = "w".red();
    let x = "x".green();
    let dash = "-".dimmed();

    let user_r = if mode & 0o400 != 0 { &r } else { &dash };
    let user_w = if mode & 0o200 != 0 { &w } else { &dash };
    let user_x = if mode & 0o100 != 0 { &x } else { &dash };

    let group_r = if mode & 0o040 != 0 { &r } else { &dash };
    let group_w = if mode & 0o020 != 0 { &w } else { &dash };
    let group_x = if mode & 0o010 != 0 { &x } else { &dash };

    let other_r = if mode & 0o004 != 0 { &r } else { &dash };
    let other_w = if mode & 0o002 != 0 { &w } else { &dash };
    let other_x = if mode & 0o001 != 0 { &x } else { &dash };

    format!(
        "{}{}{}{}{}{}{}{}{}{}",
        dir, user_r, user_w, user_x, group_r, group_w, group_x, other_r, other_w, other_x
    )
}

/// Colorizes a string representing a user or group.
pub fn colorize_user_group(name: &str) -> ColoredString {
    name.yellow().bold()
}

/// Colorizes a string representing a file size
pub fn colorize_file_size(size: &str) -> ColoredString {
    size.green().bold()
}

/// Colorizes a string representing a modified
pub fn colorize_modified(modified: &str) -> ColoredString {
    modified.blue()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip_ansi_codes(s: &str) -> String {
        let re = regex::Regex::new("\x1b\\[[0-9;]*m").unwrap();
        re.replace_all(s, "").to_string()
    }

    #[test]
    fn test_format_mode() {
        struct TestCase {
            mode: u32,
            is_dir: bool,
            expected: &'static str,
            description: &'static str,
        }

        let test_cases = vec![
            TestCase {
                mode: 0o755,
                is_dir: true,
                expected: "drwxr-xr-x",
                description: "Directory with 755",
            },
            TestCase {
                mode: 0o644,
                is_dir: false,
                expected: "-rw-r--r--",
                description: "File with 644",
            },
            TestCase {
                mode: 0o700,
                is_dir: false,
                expected: "-rwx------",
                description: "File with 700",
            },
            TestCase {
                mode: 0o000,
                is_dir: false,
                expected: "----------",
                description: "No permissions",
            },
        ];

        for case in test_cases {
            let formatted = format_mode(case.mode, case.is_dir);
            let stripped = strip_ansi_codes(&formatted);
            assert_eq!(stripped, case.expected, "Failed on: {}", case.description);
        }
    }
}
