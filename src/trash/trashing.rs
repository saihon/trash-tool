use std::fs::{self};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use chrono::Local;

use crate::trash::color::colorize_path;
use crate::trash::error::AppError;
use crate::trash::locations::{resolve_target_trash, TargetTrash};
use crate::trash::spec::{
    TRASH_INFO_DATE_FORMAT, TRASH_INFO_DATE_KEY, TRASH_INFO_HEADER, TRASH_INFO_PATH_KEY, TRASH_INFO_SUFFIX,
};
use crate::trash::url_escape::trash_spec_url_encode;

/// The starting number for the counter when resolving filename collisions in the trash.
/// This matches the behavior of popular file managers like Nautilus and Nemo.
const COLLISION_COUNTER_START: u32 = 2;

pub fn handle_move_to_trash(files: &[String]) -> Result<(), AppError> {
    let mounts = mountpoints::mountpaths()?;
    let mut trashed: Vec<String> = Vec::new();
    for file in files {
        let path = Path::new(file);
        match resolve_target_trash(path, &mounts) {
            Ok(target_trash) => {
                if let Err(e) = target_trash.ensure_structure_exists() {
                    eprintln!("Failed to prepare trash directory for '{}': {}", path.display(), e);
                    continue;
                }
                if let Err(e) = trash_item(path, &target_trash) {
                    eprintln!("Failed to trash '{}': {}", path.display(), e);
                } else {
                    trashed.push(colorize_path(&file, path).to_string());
                }
            }
            Err(e) => eprintln!("Could not determine trash location for '{}': {}", path.display(), e),
        }
    }
    println!("Trashed: {}", trashed.join(", "));
    Ok(())
}

/// Checks whether the specified file path is within the root directory of the given trash bin or within its files directory.
/// This covers both "trash-in-trash" and "dual trash" scenarios.
fn is_already_in_any_trash_location(source_path: &Path, trash_path: &Path) -> bool {
    source_path.starts_with(trash_path)
}

/// Moves a file or directory to the trash, creating a corresponding .trashinfo file.
/// This is the main entry point for trashing an item.
fn trash_item(source_path: &Path, target_trash: &TargetTrash) -> Result<(), AppError> {
    if !source_path.exists() {
        return Err(AppError::Io {
            path: source_path.to_path_buf(),
            source: io::Error::new(ErrorKind::NotFound, "source file not found"),
        });
    }
    if is_already_in_any_trash_location(source_path, target_trash.root_path()) {
        return Err(AppError::AlreadyInTrash {
            path: source_path.to_path_buf(),
        });
    }
    let trash_files_path = target_trash.files_path();
    let trash_info_path = target_trash.info_path();

    // Determine the final destination path in `Trash/files`, handling collisions.
    let dest_path = find_available_dest_path(source_path, &trash_files_path)?;

    // Create the corresponding .trashinfo file.
    create_trash_info_file(source_path, &dest_path, &trash_info_path)?;

    // Move the actual file/directory to `Trash/files`.
    // This is done *after* creating the info file, as per the spec.
    if let Err(e) = fs::rename(source_path, &dest_path) {
        // If the move fails for any reason, we must try to clean up the .trashinfo file
        // we just created to avoid an inconsistent state in the trash.
        let info_file_path = determine_info_file_path(&dest_path, &trash_info_path);
        if let Err(cleanup_err) = fs::remove_file(&info_file_path) {
            eprintln!(
                "warning: Failed to move '{}' to trash and also failed to clean up its info file '{}': {}",
                source_path.display(),
                info_file_path.display(),
                cleanup_err
            );
        }

        // Now, return the appropriate error to the caller.
        if e.kind() == ErrorKind::CrossesDevices {
            return Err(AppError::CrossDeviceMove {
                path: source_path.to_path_buf(),
            });
        } else {
            return Err(AppError::Io {
                path: source_path.to_path_buf(),
                source: e,
            });
        }
    }

    Ok(())
}

/// Finds an available path in the trash/files directory, handling name collisions.
fn find_available_dest_path(source_path: &Path, trash_files_path: &Path) -> Result<PathBuf, AppError> {
    let file_name = source_path
        .file_name()
        .ok_or_else(|| AppError::Message(format!("Source path '{}' has no filename", source_path.display())))?;
    let mut dest_path = trash_files_path.join(file_name);

    // Start counter from 2 to match the behavior observed in popular file managers
    // like Nautilus, Nemo, and Thunar. When "file.txt" exists, the next one
    // becomes "file.2.txt", not "file.1.txt".
    let mut counter = COLLISION_COUNTER_START;
    while dest_path.exists() {
        let filename_str = file_name.to_string_lossy();

        // Find the first dot to separate the base name from the full extension. This ensures that for a file like "archive.tar.gz", the counter is inserted
        // before the full extension, resulting in "archive.2.tar.gz" rather than
        // "archive.tar.2.gz", matching the behavior of common file managers.
        let (base_name, extension_part) = match filename_str.find('.') {
            Some(dot_index) if dot_index > 0 => {
                // Split at the first dot.
                (&filename_str[..dot_index], &filename_str[dot_index..])
            }
            _ => {
                // No dot found, or it's a dotfile. Treat the whole name as the base name.
                (filename_str.as_ref(), "")
            }
        };
        let new_filename = if base_name.is_empty() && !extension_part.is_empty() {
            // Handle dotfiles like ".bashrc" -> ".bashrc.2"
            format!("{}{}", filename_str, counter)
        } else {
            format!("{}.{}{}", base_name, counter, extension_part)
        };

        dest_path = trash_files_path.join(&new_filename);
        counter += 1;
    }

    Ok(dest_path)
}

/// Builds the content for a .trashinfo file.
/// This is a pure function, making it easy to test.
fn build_trash_info_content(original_abs_path: &Path, deletion_date: &str) -> String {
    format!(
        "{}\n{}={}\n{}={}\n",
        TRASH_INFO_HEADER,
        TRASH_INFO_PATH_KEY,
        trash_spec_url_encode(original_abs_path.to_string_lossy().as_ref()),
        TRASH_INFO_DATE_KEY,
        deletion_date,
    )
}

/// Determines the full path for the .trashinfo file.
/// This is a pure function, making it easy to test.
fn determine_info_file_path(dest_path: &Path, trash_info_path: &Path) -> PathBuf {
    let info_filename_osstr = dest_path.file_name().unwrap();
    let mut info_filename = info_filename_osstr.to_owned();
    info_filename.push(TRASH_INFO_SUFFIX);
    trash_info_path.join(info_filename)
}

/// Creates a .trashinfo file for a given trashed item.
fn create_trash_info_file(original_path: &Path, dest_path: &Path, trash_info_path: &Path) -> Result<(), AppError> {
    let original_abs_path = original_path.canonicalize()?;
    let deletion_date = Local::now().format(TRASH_INFO_DATE_FORMAT).to_string();
    let info_content = build_trash_info_content(&original_abs_path, &deletion_date);
    let info_file_path = determine_info_file_path(dest_path, trash_info_path);

    fs::write(info_file_path, info_content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trash::spec::{TRASH_FILES_DIR_NAME, TRASH_INFO_DIR_NAME};
    use std::fs::{self, File};
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    fn test_trash_info_constants_adhere_to_spec() {
        assert_eq!(COLLISION_COUNTER_START, 2);
    }

    #[test]
    fn test_find_available_dest_path_handles_collisions() -> Result<(), AppError> {
        let temp_trash_root = tempdir()?;
        let trash_files_path = temp_trash_root.path().join(TRASH_FILES_DIR_NAME);
        fs::create_dir_all(&trash_files_path)?;

        struct TestCase<'a> {
            description: &'a str,
            source_filename: &'a str,
            existing_files: &'a [&'a str],
            expected_filename: &'a str,
        }

        let test_cases = vec![
            TestCase {
                description: "Should return the original filename when no collision exists",
                source_filename: "test1.txt",
                existing_files: &[],
                expected_filename: "test1.txt",
            },
            TestCase {
                description: "Should append '.2' on the first collision",
                source_filename: "test2.txt",
                existing_files: &["test2.txt"],
                expected_filename: "test2.2.txt",
            },
            TestCase {
                description: "Should find the next available number, skipping existing ones",
                source_filename: "test3.txt",
                existing_files: &["test3.txt", "test3.1.txt"],
                expected_filename: "test3.2.txt",
            },
            TestCase {
                description: "Should handle collisions for files without extensions",
                source_filename: "no_ext",
                existing_files: &["no_ext"],
                expected_filename: "no_ext.2",
            },
            TestCase {
                description: "Should handle collisions for filenames with multiple dots",
                source_filename: "archive.tar.gz",
                existing_files: &["archive.tar.gz"],
                expected_filename: "archive.2.tar.gz",
            },
            TestCase {
                description: "Should handle collisions for dotfiles",
                source_filename: ".config",
                existing_files: &[".config"],
                expected_filename: ".config.2",
            },
        ];

        for case in test_cases {
            let source_path = temp_trash_root.path().join(case.source_filename);
            File::create(&source_path)?;

            for f in case.existing_files {
                File::create(trash_files_path.join(f))?;
            }

            let expected_path = trash_files_path.join(case.expected_filename);
            let actual_path = find_available_dest_path(&source_path, &trash_files_path)?;

            assert_eq!(actual_path, expected_path, "Failed on: {}", case.description);
        }

        Ok(())
    }

    #[test]
    fn test_build_trash_info_content() {
        let original_path = Path::new("/home/user/file.txt");
        let deletion_date = "2024-01-01T12:30:00";

        let expected_content = "[Trash Info]\nPath=/home/user/file.txt\nDeletionDate=2024-01-01T12:30:00\n";
        let actual_content = build_trash_info_content(original_path, deletion_date);

        assert_eq!(actual_content, expected_content);
    }

    #[test]
    fn test_determine_info_file_path() {
        let trash_info_path = Path::new("/home/user/.local/share/Trash/info");

        let dest_path1 = Path::new("/home/user/.local/share/Trash/files/file.txt");
        let expected1 = trash_info_path.join("file.txt.trashinfo");
        assert_eq!(determine_info_file_path(dest_path1, trash_info_path), expected1);

        let dest_path2 = Path::new("/home/user/.local/share/Trash/files/archive.tar.gz");
        let expected2 = trash_info_path.join("archive.tar.gz.trashinfo");
        assert_eq!(determine_info_file_path(dest_path2, trash_info_path), expected2);
    }

    #[test]
    fn test_create_trash_info_file() -> Result<(), AppError> {
        let temp_root = tempdir()?;
        let original_path = temp_root.path().join("original_file.txt");
        File::create(&original_path)?;

        let trash_root = tempdir()?;
        let trash_info_path = trash_root.path().join(TRASH_INFO_DIR_NAME);
        fs::create_dir_all(&trash_info_path)?; // ensure_structure_exists() の役割を模倣

        let dest_path = trash_root.path().join(TRASH_FILES_DIR_NAME).join("original_file.txt");

        create_trash_info_file(&original_path, &dest_path, &trash_info_path)?;

        let expected_info_file_path = trash_info_path.join(format!("original_file.txt{}", TRASH_INFO_SUFFIX));
        assert!(expected_info_file_path.exists(), ".trashinfo file should be created.");

        let info_content = fs::read_to_string(expected_info_file_path)?;
        let original_abs_path = original_path.canonicalize()?;

        let expected_start = format!("{}\n", TRASH_INFO_HEADER);
        let expected_path_line = format!("{}={}", TRASH_INFO_PATH_KEY, original_abs_path.display());
        let expected_date_prefix = format!("{}=", TRASH_INFO_DATE_KEY);
        assert!(info_content.starts_with(&expected_start));
        assert!(info_content.contains(&expected_path_line));
        assert!(info_content.contains(&expected_date_prefix));

        Ok(())
    }

    #[test]
    fn test_trash_item_success() -> Result<(), AppError> {
        let source_root = tempdir()?;
        let trash_root = tempdir()?;

        let source_path = source_root.path().join("file_to_trash.txt");
        File::create(&source_path)?;

        let original_abs_path = source_path.canonicalize()?;

        let trash_files_path = trash_root.path().join(TRASH_FILES_DIR_NAME);
        let trash_info_path = trash_root.path().join(TRASH_INFO_DIR_NAME);

        let target_trash = TargetTrash::new(
            trash_root.path().to_path_buf(),
            crate::trash::locations::TrashType::Home,
        );
        target_trash.ensure_structure_exists()?;
        trash_item(&source_path, &target_trash)?;

        assert!(!source_path.exists(), "Source file should be moved, not copied.");

        let trashed_file_path = trash_files_path.join("file_to_trash.txt");
        assert!(trashed_file_path.exists(), "File should exist in trash/files.");

        let info_file_path = trash_info_path.join(format!("file_to_trash.txt{}", TRASH_INFO_SUFFIX));
        assert!(info_file_path.exists(), ".trashinfo file should be created.");

        let info_content = fs::read_to_string(info_file_path)?;
        assert!(info_content.contains(&format!("Path={}", original_abs_path.display())));

        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn test_trash_item_cleans_up_info_file_on_rename_failure() -> Result<(), AppError> {
        let source_root = tempdir()?;
        let trash_root = tempdir()?;

        let source_path = source_root.path().join("file.txt");
        File::create(&source_path)?;

        let trash_files_path = trash_root.path().join(TRASH_FILES_DIR_NAME);
        let trash_info_path = trash_root.path().join(TRASH_INFO_DIR_NAME);

        // Make the `files` directory read-only to cause `fs::rename` to fail.
        fs::create_dir_all(&trash_files_path)?;
        let mut perms = fs::metadata(&trash_files_path)?.permissions();
        perms.set_mode(0o555); // r-xr-xr-x
        fs::set_permissions(&trash_files_path, perms)?;

        let target_trash = TargetTrash::new(
            trash_root.path().to_path_buf(),
            crate::trash::locations::TrashType::Home,
        );
        target_trash.ensure_structure_exists()?;
        let result = trash_item(&source_path, &target_trash);

        assert!(result.is_err(), "Expected trash_item to fail.");

        assert!(
            source_path.exists(),
            "Source file should still exist after a failed move."
        );

        let expected_info_path = trash_info_path.join(format!("file.txt{}", TRASH_INFO_SUFFIX));
        assert!(
            !expected_info_path.exists(),
            "The .trashinfo file should be cleaned up after a rename failure."
        );

        // Teardown
        let mut perms = fs::metadata(&trash_files_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&trash_files_path, perms)?;

        Ok(())
    }

    #[test]
    fn test_is_already_in_any_trash_location() {
        let trash_path = Path::new("/home/user/.local/share/Trash");

        // Case 1: Path is inside the 'files' directory of the trash bin.
        let path_in_files = Path::new("/home/user/.local/share/Trash/files/some_file.txt");
        assert!(
            is_already_in_any_trash_location(path_in_files, trash_path),
            "Should return true for a path inside Trash/files"
        );

        // Case 2: Path is inside the 'info' directory of the trash bin.
        let path_in_info = Path::new("/home/user/.local/share/Trash/info/some_file.txt.trashinfo");
        assert!(
            is_already_in_any_trash_location(path_in_info, trash_path),
            "Should return true for a path inside Trash/info"
        );

        // Case 3: Path is the trash root directory itself.
        let path_is_trash_root = Path::new("/home/user/.local/share/Trash");
        assert!(
            is_already_in_any_trash_location(path_is_trash_root, trash_path),
            "Should return true when the path is the trash root itself"
        );

        // Case 4: Path is completely outside the trash bin.
        let outside_path = Path::new("/home/user/documents/another_file.txt");
        assert!(
            !is_already_in_any_trash_location(outside_path, trash_path),
            "Should return false for a path outside the trash bin"
        );

        // Case 5: Path is a parent of the trash bin.
        let parent_path = Path::new("/home/user/.local/share");
        assert!(!is_already_in_any_trash_location(parent_path, trash_path));
    }

    #[test]
    fn test_trash_item_fails_if_already_in_trash() -> Result<(), AppError> {
        let trash_root = tempdir()?;
        let trash_files_path = trash_root.path().join(TRASH_FILES_DIR_NAME);
        fs::create_dir_all(&trash_files_path)?;

        // Create a file that is already inside the trash/files directory.
        let already_trashed_file = trash_files_path.join("already_trashed.txt");
        File::create(&already_trashed_file)?;

        // Attempt to trash the file that's already in the trash.
        let target_trash = TargetTrash::new(
            trash_root.path().to_path_buf(),
            crate::trash::locations::TrashType::Home,
        );
        let result = trash_item(&already_trashed_file, &target_trash);

        assert!(
            result.is_err(),
            "Expected an error when trashing an item already in the trash."
        );

        // Check for the specific error type.
        if let Err(AppError::AlreadyInTrash { path }) = result {
            assert_eq!(path, already_trashed_file, "The error should contain the correct path.");
        } else {
            panic!("Expected AppError::AlreadyInTrash, but got a different error or Ok");
        }

        Ok(())
    }
}
