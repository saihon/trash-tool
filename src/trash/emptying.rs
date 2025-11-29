use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

use crate::trash::error::AppError;
use crate::trash::listing::list_directory_contents_single_trash;
use crate::trash::locations::get_target_trash_dirs;
use crate::trash::spec::{TRASH_FILES_DIR_NAME, TRASH_INFO_DIR_NAME};

pub struct EmptyTrashOptions {
    pub all_trash: bool,
    pub no_confirm: bool,
    pub display: bool,
    pub long_format: bool,
}

pub fn handle_empty_trash(opts: EmptyTrashOptions) -> Result<(), AppError> {
    let trash_dirs = get_target_trash_dirs(opts.all_trash)?;
    if trash_dirs.is_empty() {
        return Ok(());
    }
    let mut writer = io::stdout();

    for path in trash_dirs {
        let (item_count, is_empty) = get_trash_status(&path)?;
        if is_empty {
            println!("({}): {}", item_count, path.display());
            continue;
        }

        if opts.display || opts.long_format {
            list_directory_contents_single_trash(&mut writer, &path, opts.long_format)?;
        }

        let should_empty = if opts.no_confirm {
            true
        } else {
            let mut stdin = BufReader::new(io::stdin());
            let message = format!("({}): {} - to empty? [Y/n]: ", item_count, path.display());
            confirm_input(&mut writer, &mut stdin, message)?
        };

        if should_empty {
            empty_single_trash_dir(&path)?;
            println!("Emptied trash at: {}", path.display());
        }
    }
    Ok(())
}

fn get_trash_status(trash_dir: &Path) -> Result<(usize, bool), AppError> {
    let files_dir = trash_dir.join(TRASH_FILES_DIR_NAME);
    let info_dir = trash_dir.join(TRASH_INFO_DIR_NAME);
    let files_dir_count = fs::read_dir(&files_dir)
        .map_err(|source| AppError::Io {
            path: files_dir.clone(),
            source,
        })?
        .count();
    let info_dir_count = fs::read_dir(&info_dir)
        .map_err(|source| AppError::Io {
            path: info_dir.clone(),
            source,
        })?
        .count();
    Ok((files_dir_count, files_dir_count == 0 && info_dir_count == 0))
}

fn confirm_input<W: Write, R: BufRead>(writer: &mut W, reader: &mut R, message: String) -> Result<bool, AppError> {
    let mut input = String::new();
    loop {
        write!(writer, "{}", message)?;
        writer.flush()?;
        reader.read_line(&mut input)?;
        let trimmed_input = input.trim().to_lowercase();

        if trimmed_input.is_empty() || trimmed_input == "y" || trimmed_input == "yes" {
            return Ok(true);
        } else if trimmed_input == "n" || trimmed_input == "no" {
            return Ok(false);
        }
        // If input is invalid, loop will continue and re-prompt.
        input.clear();
    }
}

/// Empties a single trash directory according to the FreeDesktop.org specification.
fn empty_single_trash_dir(trash_root: &Path) -> Result<(), AppError> {
    let targets = [TRASH_FILES_DIR_NAME, TRASH_INFO_DIR_NAME];
    let mut error_occurred = false;
    for target in targets {
        let dir = trash_root.join(target);
        if !dir.exists() {
            continue;
        }

        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) => {
                eprintln!("Failed to read directory '{}': {}", dir.display(), e);
                error_occurred = true;
                continue;
            }
        };

        for entry in entries {
            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    let result = if path.is_dir() {
                        fs::remove_dir_all(&path)
                    } else {
                        fs::remove_file(&path)
                    };

                    if let Err(e) = result {
                        eprintln!("Failed to remove '{}': {}", path.display(), e);
                        error_occurred = true;
                    }
                }
                Err(e) => {
                    eprintln!("Failed to read directory entry in '{}': {}", dir.display(), e);
                    error_occurred = true;
                }
            }
        }
    }

    if error_occurred {
        Err(AppError::Ignorable)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Cursor;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    fn test_confirm_input() {
        struct TestCase {
            input: &'static str,
            expected_result: bool,
            description: &'static str,
        }

        let test_cases = vec![
            TestCase {
                input: "\n",
                expected_result: true,
                description: "Input empty (default to yes)",
            },
            TestCase {
                input: "y\n",
                expected_result: true,
                description: "Input 'y'",
            },
            TestCase {
                input: "yes\n",
                expected_result: true,
                description: "Input 'yes'",
            },
            TestCase {
                input: "Y\n",
                expected_result: true,
                description: "Input 'Y' (case-insensitive)",
            },
            TestCase {
                input: "n\n",
                expected_result: false,
                description: "Input 'n'",
            },
            TestCase {
                input: "no\n",
                expected_result: false,
                description: "Input 'no'",
            },
        ];

        for case in test_cases {
            let mut reader = Cursor::new(case.input);
            let mut writer = Vec::new();
            let message = "Do you want to empty? [Y/n]: ".to_string();

            let result = confirm_input(&mut writer, &mut reader, message).unwrap();

            assert_eq!(result, case.expected_result, "Failed on: {}", case.description);

            let output = String::from_utf8(writer).unwrap();
            assert_eq!(output, "Do you want to empty? [Y/n]: ");
        }
    }

    #[test]
    fn test_confirm_input_invalid_then_valid() {
        let input = "maybe\nyes\n";
        let mut reader = Cursor::new(input);
        let mut writer = Vec::new();
        let message = "Do you want to empty? [Y/n]: ".to_string();

        let result = confirm_input(&mut writer, &mut reader, message).unwrap();

        assert!(result, "Should return true after an invalid input");

        let output = String::from_utf8(writer).unwrap();
        let expected_prompt = "Do you want to empty? [Y/n]: ";
        assert_eq!(
            output,
            format!("{}{}", expected_prompt, expected_prompt),
            "Should re-prompt after invalid input"
        );
    }

    #[test]
    fn test_empty_single_trash_dir() -> Result<(), AppError> {
        let trash_root = tempdir()?;

        let files_dir = trash_root.path().join(TRASH_FILES_DIR_NAME);
        let info_dir = trash_root.path().join(TRASH_INFO_DIR_NAME);
        fs::create_dir_all(&files_dir)?;
        fs::create_dir_all(&info_dir)?;

        File::create(files_dir.join("some_file.txt"))?;
        File::create(info_dir.join("some_file.txt.trashinfo"))?;

        empty_single_trash_dir(trash_root.path())?;

        // Check that the 'files' and 'info' directories still exist.
        assert!(files_dir.exists(), "'files' directory should be recreated.");
        assert!(info_dir.exists(), "'info' directory should be recreated.");

        assert_eq!(
            fs::read_dir(&files_dir)?.count(),
            0,
            "'files' directory should be empty."
        );
        assert_eq!(fs::read_dir(&info_dir)?.count(), 0, "'info' directory should be empty.");

        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn test_empty_single_trash_dir_permission_error() -> Result<(), AppError> {
        let trash_root = tempdir()?;
        let files_dir = trash_root.path().join(TRASH_FILES_DIR_NAME);
        let info_dir = trash_root.path().join(TRASH_INFO_DIR_NAME);
        fs::create_dir(&files_dir)?;
        fs::create_dir(&info_dir)?;
        let file_path = files_dir.join("some_file.txt");
        File::create(&file_path)?;

        let mut perms = fs::metadata(&files_dir)?.permissions();
        perms.set_mode(0o555); // r-xr-xr-x
        fs::set_permissions(&files_dir, perms)?;

        let result = empty_single_trash_dir(trash_root.path());

        assert!(result.is_err(), "Expected an error due to permission issues");
        if let Err(e) = result {
            assert!(matches!(e, AppError::Ignorable), "Expected AppError::Ignorable, got {:?}", e);
        } else {
            panic!("Expected an error but got Ok");
        }

        // Teardown
        let mut perms = fs::metadata(&files_dir)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&files_dir, perms)?;

        Ok(())
    }
}
