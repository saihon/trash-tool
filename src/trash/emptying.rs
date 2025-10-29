use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use crate::trash::error::AppError;
use crate::trash::spec::{TRASH_FILES_DIR_NAME, TRASH_INFO_DIR_NAME};

pub fn handle_empty_trash(trash_dirs: &[PathBuf], no_confirm: bool) -> Result<(), AppError> {
    if trash_dirs.is_empty() {
        return Ok(());
    }

    for path in trash_dirs {
        let is_confirmed = if no_confirm {
            true
        } else {
            let mut stdin = BufReader::new(io::stdin());
            let message = format!("{}: to empty? [Y/n]: ", path.display());
            confirm_input(message, &mut stdin, &mut io::stdout())?
        };
        if is_confirmed {
            empty_single_trash_dir(&path)?;
            println!("Emptied trash at: {}", path.display());
        }
    }
    Ok(())
}

fn confirm_input<R: BufRead, W: Write>(message: String, reader: &mut R, writer: &mut W) -> Result<bool, AppError> {
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
/// This involves recursively removing the `files` and `info` directories and then recreating them.
fn empty_single_trash_dir(trash_root: &Path) -> Result<(), AppError> {
    let targets = [TRASH_FILES_DIR_NAME, TRASH_INFO_DIR_NAME];
    for target in targets {
        let dir = trash_root.join(target);
        if dir.is_dir() {
            if let Err(source) = fs::remove_dir_all(&dir) {
                return Err(AppError::Io { path: dir, source });
            }
        }
        // Recreate the empty directory.
        if let Err(source) = fs::create_dir_all(&dir) {
            return Err(AppError::Io { path: dir, source });
        }
    }

    Ok(())
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

            let result = confirm_input(message, &mut reader, &mut writer).unwrap();

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

        let result = confirm_input(message, &mut reader, &mut writer).unwrap();

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
        fs::create_dir(&files_dir)?;
        File::create(files_dir.join("some_file.txt"))?;

        let mut perms = fs::metadata(trash_root.path())?.permissions();
        perms.set_mode(0o555); // r-xr-xr-x
        fs::set_permissions(trash_root.path(), perms)?;

        let result = empty_single_trash_dir(trash_root.path());

        assert!(result.is_err(), "Expected an error due to permission issues");
        if let Err(AppError::Io { path, .. }) = result {
            // The error should be about the `files` directory inside the read-only parent.
            assert_eq!(path, files_dir);
        } else {
            panic!("Expected AppError::Io, but got a different error or Ok");
        }

        // Teardown
        let mut perms = fs::metadata(trash_root.path())?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(trash_root.path(), perms)?;

        Ok(())
    }
}
