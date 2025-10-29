use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::trash::error::AppError;

use crate::trash::spec::{TRASH_FILES_DIR_NAME, TRASH_INFO_DIR_NAME};

#[cfg(unix)]
const MOUNTS_FILE_PATH: &str = "/proc/mounts";

#[derive(Debug, PartialEq)]
pub enum TrashType {
    Home,             // $XDG_DATA_HOME/Trash, $HOME/.local/share/Trash
    TopdirShared,     // $topdir/.Trash
    TopdirSharedUser, // $topdir/.Trash/$uid
    TopdirPrivate,    // $topdir/.Trash-$uid
}

pub struct TargetTrash {
    root_path: PathBuf,
    trash_type: TrashType,
}

impl TargetTrash {
    pub fn new(root_path: PathBuf, trash_type: TrashType) -> Self {
        Self { root_path, trash_type }
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    pub fn files_path(&self) -> PathBuf {
        self.root_path.join(TRASH_FILES_DIR_NAME)
    }

    pub fn info_path(&self) -> PathBuf {
        self.root_path.join(TRASH_INFO_DIR_NAME)
    }

    pub fn ensure_structure_exists(&self) -> Result<(), AppError> {
        self.create_root_dir()?;

        // The `files` and `info` directories inherit permissions from their parent, `root_path`.
        // This ensures they are secure, as the parent's restrictive permissions (e.g., 0o700)
        // effectively limit access, regardless of the process's `umask`.
        let files_path = self.files_path();
        if !files_path.exists() {
            fs::create_dir(&files_path)?;
        }

        let info_path = self.info_path();
        if !info_path.exists() {
            fs::create_dir(&info_path)?;
        }

        Ok(())
    }

    fn create_root_dir(&self) -> Result<(), AppError> {
        match self.trash_type {
            TrashType::Home => self.create_with_mode(0o700, true),
            // NOTE: This arm is currently unreachable. `get_target_trash` validates an
            // existing shared trash directory but does not create a `TargetTrash` of this
            // type. It's kept for conceptual completeness according to the specification.
            TrashType::TopdirShared => self.create_with_mode(0o1777, true),
            TrashType::TopdirSharedUser | TrashType::TopdirPrivate => self.create_with_fallback(0o700, 0o1777),
        }
    }

    /// Creates directory with a specific mode.
    fn create_with_mode(&self, mode: u32, all: bool) -> Result<(), AppError> {
        if !self.root_path.exists() {
            let create_fn = if all { fs::create_dir_all } else { fs::create_dir };
            if let Err(e) = create_fn(&self.root_path) {
                return Err(AppError::Io {
                    path: self.root_path.clone(),
                    source: e,
                });
            }
        }

        if let Err(e) = fs::set_permissions(&self.root_path, fs::Permissions::from_mode(mode)) {
            return Err(AppError::Io {
                path: self.root_path.clone(),
                source: e,
            });
        }
        Ok(())
    }

    /// Creates directory with a primary mode, falling back to another on permission error.
    fn create_with_fallback(&self, primary_mode: u32, fallback_mode: u32) -> Result<(), AppError> {
        if !self.root_path.exists() {
            if let Err(e) = fs::create_dir_all(&self.root_path) {
                // If create_dir_all fails with permission denied, it might be because
                // we can't create the parent. We let set_permissions handle the final
                // directory's permissions. But if it's another error, we fail.
                if e.kind() != std::io::ErrorKind::PermissionDenied {
                    return Err(AppError::Io {
                        path: self.root_path.clone(),
                        source: e,
                    });
                }
            }
        }

        // Try to set the primary permission.
        match fs::set_permissions(&self.root_path, fs::Permissions::from_mode(primary_mode)) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                // If permission is denied, try the fallback permission.
                fs::set_permissions(&self.root_path, fs::Permissions::from_mode(fallback_mode)).map_err(|source| {
                    AppError::Io {
                        path: self.root_path.clone(),
                        source,
                    }
                })
            }
            Err(e) => Err(AppError::Io {
                path: self.root_path.clone(),
                source: e,
            }),
        }
    }
}

pub fn get_target_trash(path_to_trash: &Path, mounts: &[PathBuf]) -> Result<TargetTrash, AppError> {
    let absolute_path = path_to_trash.canonicalize()?;
    let home_trash_path = get_local_trash_path().ok_or_else(|| AppError::Message("Home trash not found".into()))?;

    let file_mount_point = mounts
        .iter()
        .filter(|m| absolute_path.starts_with(m))
        .max_by_key(|m| m.as_os_str().len());

    let home_mount_point = mounts
        .iter()
        .filter(|m| home_trash_path.starts_with(m))
        .max_by_key(|m| m.as_os_str().len());

    // If the file is on the same filesystem as the home directory, use the home trash.
    if file_mount_point.is_some() && file_mount_point == home_mount_point {
        // Ensure the home trash directory itself is not a symbolic link for security reasons.
        if home_trash_path.is_symlink() {
            return Err(AppError::SymbolicLink { path: home_trash_path });
        }
        return Ok(TargetTrash::new(home_trash_path, TrashType::Home));
    }

    if let Some(topdir) = file_mount_point {
        let uid = users::get_current_uid();
        // Prefer shared trash `$topdir/.Trash`
        let shared_trash_base = topdir.join(".Trash");

        // Per the FreeDesktop.org Trash Specification, this application does not create
        // the shared `$topdir/.Trash` directory. Instead, it respects the administrator's
        // explicit setup. The specification states that an administrator "can create" this
        // directory, and the application's role is to "check for the presence" and
        // validate it.
        //
        // Therefore, we check if `$topdir/.Trash` exists, is a directory, and has the
        // sticky bit set. If these conditions are not met, we fall back to creating a
        // private trash directory (`$topdir/.Trash-$uid`), mirroring the behavior of
        // some file managers.
        let is_valid_shared_trash = shared_trash_base
            .symlink_metadata() // Use symlink_metadata to check the link itself, not the target.
            .map(|m| !m.is_symlink() && m.is_dir() && (m.permissions().mode() & 0o1000 != 0))
            .unwrap_or(false);

        if is_valid_shared_trash {
            let user_trash_path = shared_trash_base.join(uid.to_string());
            return Ok(TargetTrash::new(user_trash_path, TrashType::TopdirSharedUser));
        }

        // Fallback to private trash `$topdir/.Trash-$uid`
        let private_trash_path = topdir.join(format!(".Trash-{}", uid));
        return Ok(TargetTrash::new(private_trash_path, TrashType::TopdirPrivate));
    }

    // If no suitable mount point was found for the file (which is unusual but possible),
    // we cannot determine a trash location on the same filesystem.
    // Returning an error prevents an unintended cross-device move.
    Err(AppError::Message(format!(
        "Could not determine filesystem for '{}'",
        path_to_trash.display()
    )))
}

/// Finds trash directories on mounted drives by parsing /proc/mounts.
/// This is a Linux-specific implementation.
/// It checks for both shared (`$topdir/.Trash/$uid`) and private (`$topdir/.Trash-$uid`) trash directories
/// as per the FreeDesktop.org specification.
#[cfg(unix)]
fn find_trash_dirs_on_mounts(uid: u32, mounts_path: &Path) -> Vec<PathBuf> {
    let file = match File::open(mounts_path) {
        Ok(f) => f,
        Err(_) => return Vec::new(), // /proc/mounts may not exist
    };

    let uid_str = uid.to_string();

    BufReader::new(file)
        .lines()
        .filter_map(Result::ok)
        .filter_map(|line| line.split_whitespace().nth(1).map(PathBuf::from)) // Get mount point
        .filter_map(|mount_point| {
            // According to the spec, check for a shared trash directory first.
            // This is `$topdir/.Trash` with the sticky bit set.
            let shared_trash_base = mount_point.join(".Trash");
            if let Ok(metadata) = shared_trash_base.metadata() {
                // Check if it's a directory and has the sticky bit (0o1000).
                if metadata.is_dir() && (metadata.permissions().mode() & 0o1000 != 0) {
                    let user_shared_trash = shared_trash_base.join(&uid_str);
                    if user_shared_trash.is_dir() {
                        return Some(user_shared_trash); // Use `$topdir/.Trash/$uid`
                    }
                }
            }

            // If the shared trash is not valid, fall back to the private one.
            // This is `$topdir/.Trash-$uid`.
            let private_trash = mount_point.join(format!(".Trash-{}", uid));
            if private_trash.is_dir() {
                return Some(private_trash);
            }

            None
        })
        .collect()
}

/// Returns the path to the user's primary trash directory, e.g., `$HOME/.local/share/Trash`.
///
/// This function adheres to the FreeDesktop.org Trash Specification by:
/// 1. Using the path from the `$XDG_DATA_HOME` environment variable if it is set.
/// 2. Falling back to the default `$HOME/.local/share` if `$XDG_DATA_HOME` is not set.
///
/// This function is a thin wrapper around `get_local_trash_path_from` for production use.
pub fn get_local_trash_path() -> Option<PathBuf> {
    get_local_trash_path_from(dirs::data_dir())
}

/// Helper function that constructs the trash path from a given data directory `Option`.
/// This makes the logic testable by allowing injection of the data directory path.
fn get_local_trash_path_from(data_dir: Option<PathBuf>) -> Option<PathBuf> {
    data_dir.map(|mut path| {
        path.push("Trash");
        path
    })
}

pub fn find_all_trash_dirs() -> Result<Vec<PathBuf>, AppError> {
    let mut trash_dirs = Vec::new();

    match get_local_trash_path() {
        Some(local_trash) => {
            if local_trash.is_dir() {
                trash_dirs.push(local_trash);
            }
        }
        None => {}
    }

    #[cfg(unix)]
    trash_dirs.extend(find_trash_dirs_on_mounts(
        users::get_current_uid(),
        Path::new(MOUNTS_FILE_PATH),
    ));

    Ok(trash_dirs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    fn test_get_local_trash_path_from() -> Result<(), AppError> {
        let fake_data_dir = tempdir()?;

        let input = Some(fake_data_dir.path().to_path_buf());
        let expected = Some(fake_data_dir.path().join("Trash"));

        assert_eq!(
            get_local_trash_path_from(input),
            expected,
            "Should append 'Trash' to a Some(path)."
        );

        let input_none: Option<PathBuf> = None;
        assert_eq!(
            get_local_trash_path_from(input_none),
            None,
            "Should return None when input is None."
        );

        let input_empty = Some(PathBuf::from(""));
        let expected_empty = Some(PathBuf::from("Trash"));
        assert_eq!(get_local_trash_path_from(input_empty), expected_empty);

        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn test_find_trash_dirs_on_mounts() -> Result<(), AppError> {
        let uid = users::get_current_uid();
        let uid_str = uid.to_string();

        let root_dir = tempdir()?;
        let mounts_file_path = root_dir.path().join("test_mounts");
        let mut mounts_file = File::create(&mounts_file_path)?;

        // `$mount_point/.Trash` (with sticky bit) and `$mount_point/.Trash/$uid` exist.
        let mount1 = root_dir.path().join("mount1");
        fs::create_dir(&mount1)?;
        let shared_trash_base = mount1.join(".Trash");
        fs::create_dir(&shared_trash_base)?;
        fs::set_permissions(&shared_trash_base, fs::Permissions::from_mode(0o1777))?; // Set sticky bit
        let shared_trash_user = shared_trash_base.join(&uid_str);
        fs::create_dir(&shared_trash_user)?;
        writeln!(mounts_file, "none {} none 0 0", mount1.display())?;

        // `$mount_point/.Trash-$uid` exists.
        let mount2 = root_dir.path().join("mount2");
        fs::create_dir(&mount2)?;
        let private_trash = mount2.join(format!(".Trash-{}", uid));
        fs::create_dir(&private_trash)?;
        writeln!(mounts_file, "none {} none 0 0", mount2.display())?;

        // Shared Trash without sticky bit (should fall back to private)
        let mount3 = root_dir.path().join("mount3");
        fs::create_dir(&mount3)?;
        let non_sticky_shared = mount3.join(".Trash");
        fs::create_dir(&non_sticky_shared)?; // No sticky bit
        let private_trash_fallback = mount3.join(format!(".Trash-{}", uid));
        fs::create_dir(&private_trash_fallback)?;
        writeln!(mounts_file, "none {} none 0 0", mount3.display())?;

        // No valid trash directory
        let mount4 = root_dir.path().join("mount4");
        fs::create_dir(&mount4)?;
        writeln!(mounts_file, "none {} none 0 0", mount4.display())?;

        let found_dirs = find_trash_dirs_on_mounts(uid, &mounts_file_path);

        assert_eq!(found_dirs.len(), 3, "Should find three valid trash directories");

        let expected_dirs: std::collections::HashSet<PathBuf> =
            [shared_trash_user, private_trash, private_trash_fallback]
                .iter()
                .cloned()
                .collect();

        let found_dirs_set: std::collections::HashSet<PathBuf> = found_dirs.into_iter().collect();

        assert_eq!(found_dirs_set, expected_dirs);

        Ok(())
    }

    #[test]
    fn test_get_target_trash_for_home_file_uses_home_trash() -> Result<(), AppError> {
        let root = tempdir()?;
        let home = root.path().join("home/user");
        let file_in_home = home.join("file.txt");
        fs::create_dir_all(&home)?;
        File::create(&file_in_home)?;

        // Mock dirs::data_dir() to return our fake home data dir
        let home_trash_path = home.join(".local/share/Trash");
        fs::create_dir_all(&home_trash_path)?;

        // Mock get_local_trash_path to return our fake home trash
        let original_data_dir = std::env::var("XDG_DATA_HOME");
        std::env::set_var("XDG_DATA_HOME", home.join(".local/share"));

        let mounts = vec![PathBuf::from("/")];
        let target_trash = get_target_trash(&file_in_home, &mounts)?;

        assert_eq!(target_trash.root_path, home_trash_path);
        assert_eq!(target_trash.trash_type, TrashType::Home);

        // Restore env var
        if let Ok(val) = original_data_dir {
            std::env::set_var("XDG_DATA_HOME", val);
        } else {
            std::env::remove_var("XDG_DATA_HOME");
        }

        Ok(())
    }

    #[test]
    fn test_get_target_trash_for_external_file() -> Result<(), AppError> {
        let root = tempdir()?;
        let home = root.path().join("home/user");
        let usb = root.path().join("media/usb");
        let file_on_usb = usb.join("file.txt");
        fs::create_dir_all(&home)?;
        fs::create_dir_all(&usb)?;
        File::create(&file_on_usb)?;

        let uid = users::get_current_uid();

        // Mock get_local_trash_path
        let original_data_dir = std::env::var("XDG_DATA_HOME");
        std::env::set_var("XDG_DATA_HOME", home.join(".local/share"));

        let mounts = vec![PathBuf::from("/"), usb.clone()];

        // --- Case 1: No shared or private trash exists, should create private ---
        let target_trash = get_target_trash(&file_on_usb, &mounts)?;
        assert_eq!(target_trash.trash_type, TrashType::TopdirPrivate);
        assert_eq!(target_trash.root_path, usb.join(format!(".Trash-{}", uid)));

        // --- Case 2: Valid shared trash exists, should use it ---
        let shared_trash_base = usb.join(".Trash");
        fs::create_dir(&shared_trash_base)?;
        fs::set_permissions(&shared_trash_base, fs::Permissions::from_mode(0o1777))?;

        let target_trash_shared = get_target_trash(&file_on_usb, &mounts)?;
        assert_eq!(target_trash_shared.trash_type, TrashType::TopdirSharedUser);
        assert_eq!(target_trash_shared.root_path, shared_trash_base.join(uid.to_string()));

        // --- Case 3: Shared trash exists but is invalid (no sticky bit), should fall back to private ---
        fs::set_permissions(&shared_trash_base, fs::Permissions::from_mode(0o755))?;
        let target_trash_fallback = get_target_trash(&file_on_usb, &mounts)?;
        assert_eq!(target_trash_fallback.trash_type, TrashType::TopdirPrivate);

        // --- Case 4: Shared trash path is a file, should fall back to private ---
        fs::remove_dir(&shared_trash_base)?;
        File::create(&shared_trash_base)?;
        let target_trash_fallback_file = get_target_trash(&file_on_usb, &mounts)?;
        assert_eq!(target_trash_fallback_file.trash_type, TrashType::TopdirPrivate);

        // Restore env var
        if let Ok(val) = original_data_dir {
            std::env::set_var("XDG_DATA_HOME", val);
        } else {
            std::env::remove_var("XDG_DATA_HOME");
        }

        Ok(())
    }

    #[test]
    fn test_get_target_trash_symlink_check() -> Result<(), AppError> {
        let root = tempdir()?;
        let home = root.path().join("home/user");
        let real_trash = root.path().join("real_trash");
        let home_trash_path = home.join(".local/share/Trash");
        fs::create_dir_all(home.join(".local/share"))?;
        fs::create_dir_all(&real_trash)?;

        // Create a symlink for the home trash
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real_trash, &home_trash_path)?;

        // Mock get_local_trash_path
        let original_data_dir = std::env::var("XDG_DATA_HOME");
        std::env::set_var("XDG_DATA_HOME", home.join(".local/share"));

        let file_in_home = home.join("file.txt");
        File::create(&file_in_home)?;
        let mounts = vec![PathBuf::from("/")];

        #[cfg(unix)]
        {
            let result = get_target_trash(&file_in_home, &mounts);
            assert!(matches!(result, Err(AppError::SymbolicLink { .. })));
        }

        // Restore env var
        if let Ok(val) = original_data_dir {
            std::env::set_var("XDG_DATA_HOME", val);
        } else {
            std::env::remove_var("XDG_DATA_HOME");
        }

        Ok(())
    }

    #[test]
    fn test_get_target_trash_no_mount_point_found() -> Result<(), AppError> {
        let root = tempdir()?;
        let some_dir = root.path().join("some/dir");
        let file = some_dir.join("file.txt");
        fs::create_dir_all(&some_dir)?;
        File::create(&file)?;

        // Provide an empty list of mounts, so none will be found for the file.
        let mounts = vec![];
        let result = get_target_trash(&file, &mounts);

        assert!(
            matches!(result, Err(AppError::Message(_))),
            "Should return an error when no mount point can be determined"
        );

        Ok(())
    }

    #[test]
    fn test_ensure_structure_exists() -> Result<(), AppError> {
        let root = tempdir()?;
        let trash_path = root.path().join("TestTrash");

        // --- Case 1: Home Trash ---
        let home_trash = TargetTrash::new(trash_path.clone(), TrashType::Home);
        home_trash.ensure_structure_exists()?;

        assert!(trash_path.exists());
        assert!(trash_path.join("files").exists());
        assert!(trash_path.join("info").exists());
        #[cfg(unix)]
        assert_eq!(fs::metadata(&trash_path)?.permissions().mode() & 0o777, 0o700);

        // Run again to test idempotency
        home_trash.ensure_structure_exists()?;
        assert!(trash_path.exists());

        fs::remove_dir_all(&trash_path)?;

        // --- Case 2: Private Trash ---
        let private_trash = TargetTrash::new(trash_path.clone(), TrashType::TopdirPrivate);
        private_trash.ensure_structure_exists()?;

        assert!(trash_path.exists());
        assert!(trash_path.join("files").exists());
        assert!(trash_path.join("info").exists());
        #[cfg(unix)]
        assert_eq!(fs::metadata(&trash_path)?.permissions().mode() & 0o777, 0o700);

        fs::remove_dir_all(&trash_path)?;

        // --- Case 3: Shared User Trash ---
        // We need a writable parent for the fallback test later.
        let shared_parent = root.path().join("SharedParent");
        fs::create_dir(&shared_parent)?;
        let shared_user_path = shared_parent.join("1000");

        let shared_user_trash = TargetTrash::new(shared_user_path.clone(), TrashType::TopdirSharedUser);
        shared_user_trash.ensure_structure_exists()?;

        assert!(shared_user_path.exists());
        assert!(shared_user_path.join("files").exists());
        assert!(shared_user_path.join("info").exists());
        #[cfg(unix)]
        assert_eq!(fs::metadata(&shared_user_path)?.permissions().mode() & 0o777, 0o700);

        Ok(())
    }
}
