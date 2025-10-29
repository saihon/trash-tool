use std::fs;
use std::path::Path;

const CONFIG_EXTENSIONS: &[&str] = &[
    "toml", "yaml", "yml", "json", "conf", "ini", "env", "gradle", "xml", "cfg",
];

const CONFIG_FILENAMES: &[&str] = &[
    "makefile",
    "cargo.toml",
    "package.json",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "composer.json",
    "pom.xml",
    "build.gradle",
    "gemfile",
    "pipfile",
    "pipfile.lock",
    "requirements.txt",
    "pyproject.toml",
    "setup.py",
    "setup.cfg",
    "dockerfile",
    "docker-compose.yml",
    "license",
    "license.txt",
    ".editorconfig",
    ".gitignore",
    ".gitattributes",
    ".gitmodules",
    ".prettierrc",
    "tsconfig.json",
    "jsconfig.json",
    "webpack.config.js",
    "vite.config.js",
    "rollup.config.js",
    "vagrantfile",
];

const ARCHIVE_EXTENSIONS: &[&str] = &[
    "zip", "tar", "gz", "bz2", "xz", "tgz", "tbz2", "7z", "rar", "deb", "iso", "zst",
];
const DOCUMENT_EXTENSIONS: &[&str] = &[
    "md", "txt", "doc", "docx", "pdf", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "rtf", "epub", "csv",
];
const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "bmp", "svg", "webp", "heic", "heif", "tiff", "tif", "ico", "avif",
];
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "mov", "avi", "webm", "mpeg", "mpg", "flv", "wmv", "3gp"];
const MUSIC_EXTENSIONS: &[&str] = &["mp3", "flac", "m4a", "wav", "ogg", "aac", "alac", "aiff", "opus"];

/// Represents the classified type of a file or directory.
#[derive(Debug, PartialEq)]
pub enum FileType {
    Directory,
    Executable,
    Archive,
    Config,
    Document,
    Image,
    Video,
    Music,
    Other,
}

/// Determines the `FileType` of a given path.
pub fn get_file_type(path: &Path) -> FileType {
    if path.is_dir() {
        return FileType::Directory;
    }

    if is_executable(path) {
        return FileType::Executable;
    }

    let filename_lower = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();

    // Match by exact filename, prefix, suffix, or extension
    if CONFIG_EXTENSIONS.contains(&extension.as_str())
        || CONFIG_FILENAMES.contains(&filename_lower.as_str())
        || filename_lower.starts_with(".env")
        || filename_lower.ends_with(".config.js")
        || filename_lower.ends_with(".config.mjs")
        || filename_lower.ends_with(".config.ts")
        || filename_lower.ends_with("rc")
    {
        return FileType::Config;
    }

    if ARCHIVE_EXTENSIONS.contains(&extension.as_str()) {
        return FileType::Archive;
    } else if DOCUMENT_EXTENSIONS.contains(&extension.as_str()) {
        return FileType::Document;
    } else if IMAGE_EXTENSIONS.contains(&extension.as_str()) {
        return FileType::Image;
    } else if VIDEO_EXTENSIONS.contains(&extension.as_str()) {
        return FileType::Video;
    } else if MUSIC_EXTENSIONS.contains(&extension.as_str()) {
        return FileType::Music;
    }

    // If no specific type was found
    FileType::Other
}

/// Checks if a file is executable (Unix-like OS only).
#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Fallback for non-Unix systems where executable permissions are not standard.
#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_get_file_type_by_extension_and_name() {
        struct TestCase {
            path: &'static str,
            expected: FileType,
            description: &'static str,
        }

        let test_cases = vec![
            // Config files
            TestCase {
                path: "config.toml",
                expected: FileType::Config,
                description: "TOML config",
            },
            TestCase {
                path: "Makefile",
                expected: FileType::Config,
                description: "Makefile exact name",
            },
            TestCase {
                path: ".env.local",
                expected: FileType::Config,
                description: ".env prefix",
            },
            TestCase {
                path: "pylintrc",
                expected: FileType::Config,
                description: "rc suffix",
            },
            // Archives
            TestCase {
                path: "archive.zip",
                expected: FileType::Archive,
                description: "zip archive",
            },
            TestCase {
                path: "data.tar.gz",
                expected: FileType::Archive,
                description: "tar.gz archive",
            },
            // Documents
            TestCase {
                path: "README.md",
                expected: FileType::Document,
                description: "Markdown document",
            },
            TestCase {
                path: "report.pdf",
                expected: FileType::Document,
                description: "PDF document",
            },
            // Media
            TestCase {
                path: "photo.jpeg",
                expected: FileType::Image,
                description: "JPEG image",
            },
            TestCase {
                path: "movie.mkv",
                expected: FileType::Video,
                description: "MKV video",
            },
            TestCase {
                path: "song.flac",
                expected: FileType::Music,
                description: "FLAC music",
            },
            // Edge cases
            TestCase {
                path: ".bashrc",
                expected: FileType::Config,
                description: "Dotfile with rc suffix",
            },
            TestCase {
                path: "archive.tar.gz",
                expected: FileType::Archive,
                description: "Double extension archive",
            },
            // Other
            TestCase {
                path: "unknown.file",
                expected: FileType::Other,
                description: "Unknown extension",
            },
            TestCase {
                path: "no_extension",
                expected: FileType::Other,
                description: "File with no extension",
            },
        ];

        for case in test_cases {
            let file_type = get_file_type(Path::new(case.path));
            assert_eq!(
                file_type, case.expected,
                "Test failed for '{}'. Description: {}",
                case.path, case.description
            );
        }
    }
}
