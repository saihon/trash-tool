/// Constants for the .trashinfo file format, as per the FreeDesktop.org spec.
pub const TRASH_INFO_HEADER: &str = "[Trash Info]";
pub const TRASH_INFO_PATH_KEY: &str = "Path";
pub const TRASH_INFO_DATE_KEY: &str = "DeletionDate";
pub const TRASH_INFO_EXTENSION: &str = "trashinfo";
pub const TRASH_INFO_SUFFIX: &str = ".trashinfo";
pub const TRASH_INFO_DATE_FORMAT: &str = "%Y-%m-%dT%H:%M:%S";
pub const TRASH_FILES_DIR_NAME: &str = "files";
pub const TRASH_INFO_DIR_NAME: &str = "info";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants_adhere_to_spec() {
        assert_eq!(TRASH_INFO_HEADER, "[Trash Info]");
        assert_eq!(TRASH_INFO_PATH_KEY, "Path");
        assert_eq!(TRASH_INFO_DATE_KEY, "DeletionDate");
        assert_eq!(TRASH_INFO_EXTENSION, "trashinfo");
        assert_eq!(TRASH_INFO_SUFFIX, ".trashinfo");
        assert_eq!(TRASH_INFO_DATE_FORMAT, "%Y-%m-%dT%H:%M:%S");
        assert_eq!(TRASH_FILES_DIR_NAME, "files");
        assert_eq!(TRASH_INFO_DIR_NAME, "info");
    }
}
