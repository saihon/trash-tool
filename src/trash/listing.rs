use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use humansize::{format_size, BINARY};
use term_grid::{Cell, Direction, Filling, Grid, GridOptions};

use super::color::{colorize_file_size, colorize_modified, colorize_path, colorize_user_group, format_mode};
use crate::trash::color::colorize_trash_directory;
use crate::trash::error::AppError;
use crate::trash::locations::get_target_trash_dirs;
use crate::trash::spec::TRASH_FILES_DIR_NAME;

#[cfg(unix)]
use {
    std::os::unix::fs::MetadataExt,
    users::{get_group_by_gid, get_user_by_uid},
};

pub fn handle_display_trash(all_trash: bool, long_format: bool) -> Result<(), AppError> {
    let trash_dirs = get_target_trash_dirs(all_trash)?;
    if trash_dirs.is_empty() {
        return Err(AppError::NoTrashDirectories);
    }
    let mut writer = io::stdout();
    for path in trash_dirs.iter() {
        list_directory_contents_single_trash(&mut writer, path, long_format)?;
    }
    Ok(())
}

pub fn list_directory_contents_single_trash<W: Write>(
    writer: &mut W,
    trash_dir: &Path,
    long_format: bool,
) -> Result<(), AppError> {
    let files_dir = trash_dir.join(TRASH_FILES_DIR_NAME);
    print_absolute_path(writer, &files_dir)?;
    if long_format {
        list_directory_contents_long(writer, &files_dir)?;
    } else {
        list_directory_contents(writer, &files_dir)?;
    }
    Ok(())
}

fn print_absolute_path<W: Write>(writer: &mut W, dir_path: &Path) -> Result<(), AppError> {
    let absolute_path = fs::canonicalize(dir_path).unwrap_or_else(|_| dir_path.to_path_buf());
    writeln!(
        writer,
        "{}",
        colorize_trash_directory(&absolute_path.display().to_string())
    )?;
    Ok(())
}

fn get_dir_entry_paths(dir_path: &Path) -> Result<Vec<PathBuf>, AppError> {
    let entries = match fs::read_dir(dir_path) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(AppError::Io {
                path: dir_path.to_path_buf(),
                source,
            })
        }
    };

    entries
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::from)
}

fn list_directory_contents<W: Write>(writer: &mut W, dir_path: &Path) -> Result<(), AppError> {
    let entries = get_dir_entry_paths(dir_path)?;

    if entries.is_empty() {
        writeln!(writer, "  (empty)")?;
        return Ok(());
    };

    if let Some(width) = term_size::dimensions().map(|(w, _)| w) {
        let mut grid = Grid::new(GridOptions {
            direction: Direction::TopToBottom,
            filling: Filling::Spaces(2),
        });

        for entry in entries {
            let path = entry;
            let filename = path
                .file_name()
                .map(|s| s.to_string_lossy())
                .unwrap_or_else(|| "(Unknown)".into());

            let colored_string = colorize_path(filename.as_ref(), path.as_path());

            grid.add(Cell {
                contents: colored_string.to_string(),
                width: filename.chars().count(),
            });
        }

        if let Some(display) = grid.fit_into_width(width) {
            write!(writer, "{}", display)?;
        }
    }

    Ok(())
}

fn list_directory_contents_long<W: Write>(writer: &mut W, dir_path: &Path) -> Result<(), AppError> {
    let entries = get_dir_entry_paths(dir_path)?;

    if entries.is_empty() {
        writeln!(writer, "  (empty)")?;
        return Ok(());
    };

    for entry in entries {
        let path = entry;
        let metadata = std::fs::metadata(&path).map_err(|source| AppError::Io {
            path: path.clone(),
            source,
        })?;

        #[cfg(unix)]
        {
            let mode_str = format_mode(metadata.mode(), metadata.is_dir());
            let nlink = metadata.nlink();
            let user = get_user_by_uid(metadata.uid())
                .map(|u| u.name().to_string_lossy().into_owned())
                .unwrap_or_else(|| metadata.uid().to_string());
            let group = get_group_by_gid(metadata.gid())
                .map(|g| g.name().to_string_lossy().into_owned())
                .unwrap_or_else(|| metadata.gid().to_string());
            let size = format_size(metadata.len(), BINARY);
            let modified: DateTime<Local> = DateTime::from(metadata.modified()?);
            let filename = path.file_name().unwrap().to_string_lossy();

            writeln!(
                writer,
                "{} {:>2} {:<7} {:<7} {:>10} {} {}",
                mode_str,
                nlink,
                colorize_user_group(&user),
                colorize_user_group(&group),
                colorize_file_size(size.as_str()),
                colorize_modified(modified.format("%b %d %H:%M").to_string().as_str()),
                colorize_path(&filename, &path)
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    // Helper to remove ANSI color codes for stable string comparison in tests.
    fn strip_ansi(s: &str) -> String {
        let re = regex::Regex::new("\x1b\\[[0-9;]*m").unwrap();
        re.replace_all(s, "").to_string()
    }

    #[test]
    #[cfg(unix)]
    fn test_list_directory_contents_long() -> Result<(), AppError> {
        let temp_dir = tempdir()?;
        let files_dir = temp_dir.path();

        let file_path = files_dir.join("test-file.txt");
        File::create(&file_path)?;

        // Explicitly set file permissions to make the test environment-independent.
        let mut perms = fs::metadata(&file_path)?.permissions();
        perms.set_mode(0o644); // -rw-r--r--
        fs::set_permissions(&file_path, perms)?;

        // Get current user/group for assertion.
        let uid = users::get_current_uid();
        let user = users::get_user_by_uid(uid)
            .map(|u| u.name().to_string_lossy().into_owned())
            .unwrap_or_else(|| uid.to_string());
        let gid = users::get_current_gid();
        let group = users::get_group_by_gid(gid)
            .map(|g| g.name().to_string_lossy().into_owned())
            .unwrap_or_else(|| gid.to_string());

        let mut output_buffer = Vec::new();
        list_directory_contents_long(&mut output_buffer, files_dir)?;

        let output = String::from_utf8(output_buffer)?;
        let stripped_output = strip_ansi(&output);

        assert!(stripped_output.contains("-rw-r--r--"));
        assert!(stripped_output.contains(&user));
        assert!(stripped_output.contains(&group));
        assert!(stripped_output.contains("test-file.txt"));

        Ok(())
    }

    #[test]
    fn test_list_directory_contents() -> Result<(), AppError> {
        let temp_dir_with_files = tempdir()?;
        let files_dir = temp_dir_with_files.path();

        File::create(files_dir.join("file1.txt"))?;
        File::create(files_dir.join("another-file.log"))?;

        let mut output_buffer = Vec::new();
        list_directory_contents(&mut output_buffer, files_dir)?;

        let output = String::from_utf8(output_buffer)?;
        let stripped_output = strip_ansi(&output);

        assert!(
            stripped_output.contains("file1.txt"),
            "Should contain the first filename"
        );
        assert!(
            stripped_output.contains("another-file.log"),
            "Should contain the second filename"
        );

        let temp_dir_empty = tempdir()?;
        let empty_dir = temp_dir_empty.path();

        let mut output_buffer_empty = Vec::new();
        list_directory_contents(&mut output_buffer_empty, empty_dir)?;

        let output_empty = String::from_utf8(output_buffer_empty)?;
        let stripped_output_empty = strip_ansi(&output_empty);

        assert!(
            stripped_output_empty.contains("(empty)"),
            "Should display '(empty)' for an empty directory"
        );

        Ok(())
    }

    #[test]
    fn test_list_on_non_existent_directory() -> Result<(), AppError> {
        let temp_dir = tempdir()?;
        let non_existent_path = temp_dir.path().join("does-not-exist");

        let mut output_buffer = Vec::new();
        let result = list_directory_contents(&mut output_buffer, &non_existent_path);

        assert!(
            result.is_ok(),
            "Should not return an error for a non-existent directory"
        );
        let output = String::from_utf8(output_buffer)?;
        let stripped_output = strip_ansi(&output);
        assert!(
            stripped_output.contains("(empty)"),
            "Should display '(empty)' for a non-existent directory"
        );

        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn test_list_directory_permission_error() -> Result<(), AppError> {
        let temp_dir = tempdir()?;
        let unreadable_dir = temp_dir.path().join("unreadable");
        fs::create_dir(&unreadable_dir)?;

        // Set permissions to 0o000 (no read/write/execute) to trigger an I/O error.
        let mut perms = fs::metadata(&unreadable_dir)?.permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&unreadable_dir, perms)?;

        let mut output_buffer = Vec::new();
        let result = list_directory_contents(&mut output_buffer, &unreadable_dir);

        assert!(result.is_err(), "Expected an I/O error due to permissions");
        if let Err(AppError::Io { path, .. }) = result {
            assert_eq!(path, unreadable_dir);
        } else {
            panic!("Expected AppError::Io, but got a different error or Ok");
        }

        let mut perms = fs::metadata(&unreadable_dir)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&unreadable_dir, perms)?;

        Ok(())
    }
}
