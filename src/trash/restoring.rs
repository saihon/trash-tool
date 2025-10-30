use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use once_cell::sync::Lazy;
use regex::Regex;
use skim::{prelude::*, SkimOptions};

use crate::trash::error::AppError;
use crate::trash::locations::find_all_trash_dirs;
use crate::trash::spec::{
    TRASH_FILES_DIR_NAME, TRASH_INFO_DATE_KEY, TRASH_INFO_DIR_NAME, TRASH_INFO_EXTENSION, TRASH_INFO_PATH_KEY,
    TRASH_INFO_SUFFIX,
};
use crate::trash::url_escape::trash_spec_url_decode;

#[derive(Debug, Clone)]
struct TrashEntry {
    // Path to the file/dir inside `Trash/files`
    trashed_path: PathBuf,
    // Path to the `.trashinfo` file inside `Trash/info`
    info_path: PathBuf,
    // Original path of the item
    original_path: PathBuf,
    // Deletion date string
    deletion_date: String,
}

impl SkimItem for TrashEntry {
    fn text(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "{}  {} <= {}",
            self.deletion_date,
            self.original_path.display(),
            self.trashed_path.display()
        ))
    }
}

static PATH_RE: Lazy<Regex> = Lazy::new(|| Regex::new(&format!(r"^{}=(.*)$", TRASH_INFO_PATH_KEY)).unwrap());
static DATE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(&format!(r"^{}=(.*)$", TRASH_INFO_DATE_KEY)).unwrap());

/// Finds all trash entries by scanning .trashinfo files.
fn find_trash_entries() -> Result<Vec<TrashEntry>, AppError> {
    let trash_dirs = find_all_trash_dirs()?;
    find_trash_entries_in_dirs(&trash_dirs)
}

fn get_capture(re: &Regex, line: &str) -> Option<String> {
    re.captures(line)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// Helper function that finds trash entries in a given list of directories.
fn find_trash_entries_in_dirs(trash_dirs: &[PathBuf]) -> Result<Vec<TrashEntry>, AppError> {
    let mut entries = Vec::new();

    for trash_dir in trash_dirs {
        let info_dir = trash_dir.join(TRASH_INFO_DIR_NAME);
        if !info_dir.is_dir() {
            continue;
        }

        let dir_entries = fs::read_dir(&info_dir).map_err(|source| AppError::Io {
            path: info_dir.clone(),
            source,
        })?;

        for entry in dir_entries {
            let entry = entry.map_err(|source| AppError::Io {
                path: info_dir.clone(),
                source,
            })?;
            let info_path = entry.path();
            if info_path.extension().and_then(|s| s.to_str()) != Some(TRASH_INFO_EXTENSION) {
                continue;
            }

            let content = fs::read_to_string(&info_path).map_err(|source| AppError::Io {
                path: info_path.clone(),
                source,
            })?;
            let mut original_path_str = None;
            let mut deletion_date = None;

            for line in content.lines() {
                if original_path_str.is_none() {
                    original_path_str = get_capture(&PATH_RE, line);
                }
                if deletion_date.is_none() {
                    deletion_date = get_capture(&DATE_RE, line);
                }
            }

            if let (Some(original_path_str), Some(deletion_date)) = (original_path_str, deletion_date) {
                // Decode the URL-escaped path from the .trashinfo file.
                match trash_spec_url_decode(&original_path_str) {
                    Ok(decoded_path) => {
                        let info_filename = info_path.file_name().unwrap().to_string_lossy();
                        let base_filename = info_filename.strip_suffix(TRASH_INFO_SUFFIX).unwrap_or(&info_filename);

                        let trashed_path = trash_dir.join(TRASH_FILES_DIR_NAME).join(base_filename);

                        entries.push(TrashEntry {
                            trashed_path,
                            info_path: info_path.clone(),
                            original_path: PathBuf::from(decoded_path),
                            deletion_date,
                        });
                    }
                    Err(e) => {
                        // If decoding fails, the .trashinfo file is likely corrupt.
                        // Warn the user and skip this entry.
                        eprintln!(
                            "warning: Failed to decode path from '{}': {}. Skipping entry.",
                            info_path.display(),
                            e
                        );
                    }
                }
            }
        }
    }
    Ok(entries)
}

/// Interactively select and restore items from the trash.
pub fn handle_interactive_restore(mut skim_options: SkimOptions) -> Result<(), AppError> {
    let entries = find_trash_entries()?;
    if entries.is_empty() {
        println!("Trash is empty. Nothing to restore.");
        return Ok(());
    }

    let (tx_skim, rx_skim): (SkimItemSender, SkimItemReceiver) = unbounded();
    for entry in &entries {
        let _ = tx_skim.send(Arc::new(entry.clone()));
    }
    drop(tx_skim);

    // Prepend essential keybindings at the beginning of the list.
    // This ensures that any user-defined bindings for the same keys (Environment
    // variables or CLI arguments) will take precedence, as skim processes them later.
    let default_binds = ["Enter:accept", "Esc:abort", "ctrl-c:abort"].map(String::from);
    skim_options.bind.splice(0..0, default_binds);

    let skim_output = Skim::run_with(&skim_options, Some(rx_skim));

    let mut messages: Vec<String> = vec![];
    let mut had_errors = false;

    match skim_output {
        Some(output) if !output.is_abort => {
            if output.selected_items.is_empty() {
                // println!("No items selected.");
            } else {
                for item in output.selected_items {
                    let entry = (*item).as_any().downcast_ref::<TrashEntry>().unwrap();
                    match restore_item(entry) {
                        Ok(path) => {
                            messages.push(format!("Restored: {}", path.display()));
                            // println!("Restored: {}", path.display())
                        }
                        Err(e) => {
                            messages.push(format!("Failed to restore '{}': {}", entry.original_path.display(), e));
                            had_errors = true;
                            // eprintln!("Failed to restore '{}': {}", entry.original_path.display(), e);
                        }
                    }
                }
            }
        }
        _ => {
            // User cancelled (e.g., with Esc, Ctrl-C).
            // println!("Restore cancelled.");
        }
    }

    if !skim_options.no_clear {
        print!("\x1B[2J\x1B[H");
    }
    for message in messages {
        println!("{}", message);
    }
    if had_errors {
        return Err(AppError::Ignorable);
    }
    Ok(())
}

/// Restores a single TrashEntry.
/// Returns the path of the restored item on success.
fn restore_item(entry: &TrashEntry) -> Result<PathBuf, AppError> {
    if entry.original_path.exists() {
        return Err(AppError::RestoreCollision {
            path: entry.original_path.clone(),
        });
    }

    if let Some(parent) = entry.original_path.parent() {
        if let Err(source) = fs::create_dir_all(parent) {
            return Err(AppError::Io {
                path: parent.to_path_buf(),
                source,
            });
        }
    }

    if !entry.trashed_path.exists() {
        return Err(AppError::TrashedItemNotFound {
            path: entry.trashed_path.clone(),
        });
    }

    // Move the file from the trash back to its original location.
    if let Err(source) = fs::rename(&entry.trashed_path, &entry.original_path) {
        // TODO: Implement cross-device move logic here if `rename` fails.
        return Err(AppError::Io {
            path: entry.trashed_path.clone(),
            source,
        });
    }

    // Clean up the corresponding .trashinfo file.
    if let Err(source) = fs::remove_file(&entry.info_path) {
        // This is not a critical failure, but we should warn the user.
        eprintln!(
            "warning: Restored '{}' but failed to remove its info file '{}': {}",
            entry.original_path.display(),
            entry.info_path.display(),
            source
        );
    }

    Ok(entry.original_path.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    fn test_trash_entry_skim_item_text() {
        // Create a sample TrashEntry.
        let entry = TrashEntry {
            trashed_path: PathBuf::from("/trash/files/test.txt"),
            info_path: PathBuf::from("/trash/info/test.txt.trashinfo"),
            original_path: PathBuf::from("/home/user/documents/test.txt"),
            deletion_date: "2024-01-01T12:00:00".to_string(),
        };

        // Define the expected output format.
        let expected_text = "2024-01-01T12:00:00  /home/user/documents/test.txt <= /trash/files/test.txt";
        // Call the `text` method and assert that the output is correct.
        assert_eq!(
            entry.text(),
            expected_text,
            "The SkimItem text format should match the expected output."
        );
    }

    #[test]
    fn test_restore_item_success() -> Result<(), AppError> {
        let trash_root = tempdir()?;
        let original_root = tempdir()?;

        let trashed_path = trash_root.path().join(TRASH_FILES_DIR_NAME).join("test.txt");
        let info_path = trash_root.path().join(TRASH_INFO_DIR_NAME).join("test.txt.trashinfo");
        fs::create_dir_all(trashed_path.parent().unwrap())?;
        fs::create_dir_all(info_path.parent().unwrap())?; // This line is redundant but harmless
        File::create(&trashed_path)?;
        File::create(&info_path)?;

        let original_path = original_root.path().join("documents/test.txt");

        let entry = TrashEntry {
            trashed_path,
            info_path,
            original_path: original_path.clone(),
            deletion_date: String::new(),
        };

        let restored_path = restore_item(&entry)?;

        assert_eq!(restored_path, original_path);
        // Check that the file was actually moved to the original path.
        assert!(original_path.exists());
        // Check that the parent directory was created.
        assert!(original_path.parent().unwrap().exists());
        // Check that the source file in trash is gone.
        assert!(!entry.trashed_path.exists());
        // Check that the info file was cleaned up.
        assert!(!entry.info_path.exists());

        Ok(())
    }

    #[test]
    fn test_restore_item_fails_if_destination_exists() -> Result<(), AppError> {
        let trash_root = tempdir()?;
        let original_root = tempdir()?;

        let trashed_path = trash_root.path().join(TRASH_FILES_DIR_NAME).join("test.txt");
        fs::create_dir_all(trashed_path.parent().unwrap())?;
        File::create(&trashed_path)?;

        let original_path = original_root.path().join("test.txt");
        File::create(&original_path)?;

        let entry = TrashEntry {
            trashed_path,
            info_path: trash_root.path().join(TRASH_INFO_DIR_NAME).join("test.txt.trashinfo"),
            original_path,
            deletion_date: String::new(),
        };

        let result = restore_item(&entry);
        assert!(result.is_err());
        if let Some(err) = result.err() {
            assert!(
                matches!(err, AppError::RestoreCollision { .. }),
                "Expected RestoreCollision error"
            );
        } else {
            panic!("Expected an error but got Ok");
        }

        assert!(entry.original_path.exists());
        assert!(entry.trashed_path.exists());

        Ok(())
    }

    #[test]
    fn test_find_trash_entries_in_dirs() -> Result<(), AppError> {
        let trash_root = tempdir()?;
        let files_dir = trash_root.path().join(TRASH_FILES_DIR_NAME);
        let info_dir = trash_root.path().join(TRASH_INFO_DIR_NAME);
        fs::create_dir_all(&files_dir)?;
        fs::create_dir_all(&info_dir)?;

        // A valid entry
        let mut info1 = File::create(info_dir.join(format!("file1.txt{}", TRASH_INFO_SUFFIX)))?;
        info1.write_all(b"[Trash Info]\nPath=/home/user/file1.txt\nDeletionDate=2024-01-01T12:00:00\n")?;
        File::create(files_dir.join("file1.txt"))?;

        // A valid entry with a complex name (dots in filename)
        let mut info2 = File::create(info_dir.join(format!("archive.tar.gz{}", TRASH_INFO_SUFFIX)))?;
        info2.write_all(b"[Trash Info]\nPath=/home/user/archive.tar.gz\nDeletionDate=2024-01-02T12:00:00\n")?;
        File::create(files_dir.join("archive.tar.gz"))?;

        // An invalid .trashinfo file (missing Path)
        let mut info3 = File::create(info_dir.join(format!("incomplete.txt{}", TRASH_INFO_SUFFIX)))?;
        info3.write_all(b"[Trash Info]\nDeletionDate=2024-01-03T12:00:00\n")?;

        // A file that is not a .trashinfo file
        File::create(info_dir.join("not-a-trashinfo.log"))?;

        let trash_dirs = vec![trash_root.path().to_path_buf()];
        let entries = find_trash_entries_in_dirs(&trash_dirs)?;

        assert_eq!(entries.len(), 2, "Should find exactly two valid entries");

        let mut sorted_entries = entries;
        sorted_entries.sort_by(|a, b| a.deletion_date.cmp(&b.deletion_date));

        // Verify the first entry
        let entry1 = &sorted_entries[0];
        assert_eq!(entry1.original_path, PathBuf::from("/home/user/file1.txt"));
        assert_eq!(entry1.trashed_path, files_dir.join("file1.txt"));
        assert_eq!(
            entry1.info_path,
            info_dir.join(format!("file1.txt{}", TRASH_INFO_SUFFIX))
        );
        assert_eq!(entry1.deletion_date, "2024-01-01T12:00:00");

        // Verify the second entry (complex name)
        let entry2 = &sorted_entries[1];
        assert_eq!(entry2.original_path, PathBuf::from("/home/user/archive.tar.gz"));
        assert_eq!(entry2.trashed_path, files_dir.join("archive.tar.gz"));
        assert_eq!(
            entry2.info_path,
            info_dir.join(format!("archive.tar.gz{}", TRASH_INFO_SUFFIX))
        );

        Ok(())
    }

    #[test]
    fn test_restore_item_fails_if_trashed_file_is_missing() -> Result<(), AppError> {
        let trash_root = tempdir()?;
        let original_root = tempdir()?;

        let info_path = trash_root
            .path()
            .join(TRASH_INFO_DIR_NAME)
            .join("missing_file.txt.trashinfo");
        fs::create_dir_all(info_path.parent().unwrap())?;
        File::create(&info_path)?;

        let entry = TrashEntry {
            trashed_path: trash_root.path().join(TRASH_FILES_DIR_NAME).join("missing_file.txt"),
            info_path,
            original_path: original_root.path().join("missing_file.txt"),
            deletion_date: String::new(),
        };

        let result = restore_item(&entry);
        assert!(
            result.is_err(),
            "Expected an error because the source file in trash is missing"
        );

        if let Err(AppError::TrashedItemNotFound { path }) = result {
            assert_eq!(path, entry.trashed_path);
        } else {
            panic!("Expected AppError::TrashedItemNotFound, but got a different error or Ok");
        }

        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn test_restore_item_succeeds_even_if_info_cleanup_fails() -> Result<(), AppError> {
        let trash_root = tempdir()?;
        let original_root = tempdir()?;

        let trashed_path = trash_root.path().join(TRASH_FILES_DIR_NAME).join("test.txt");
        let info_path = trash_root.path().join(TRASH_INFO_DIR_NAME).join("test.txt.trashinfo");
        let info_dir = info_path.parent().unwrap();

        fs::create_dir_all(trashed_path.parent().unwrap())?;
        fs::create_dir_all(info_dir)?;
        File::create(&trashed_path)?;
        File::create(&info_path)?;

        let entry = TrashEntry {
            trashed_path: trashed_path.clone(),
            info_path: info_path.clone(),
            original_path: original_root.path().join("test.txt"),
            deletion_date: String::new(),
        };

        // Make the `info` directory read-only to prevent `remove_file` from succeeding.
        let mut perms = fs::metadata(info_dir)?.permissions();
        perms.set_mode(0o555); // r-xr-xr-x
        fs::set_permissions(info_dir, perms)?;

        let result = restore_item(&entry);

        assert!(result.is_ok(), "Restore should succeed even if info file cleanup fails");
        // The original file should be restored.
        assert!(entry.original_path.exists());
        // The source file in trash should be gone.
        assert!(!trashed_path.exists());
        // The info file should *still exist* because it couldn't be deleted.
        assert!(info_path.exists());

        // Teardown
        fs::set_permissions(info_dir, fs::Permissions::from_mode(0o755))?;
        Ok(())
    }
}
