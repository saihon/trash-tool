use std::str::Utf8Error;

use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};

// Defines the encoding rules to be applied to the `Path` key in the Trash specification.
// Based on RFC 2396 / 3986, this specifies characters that should normally be escaped in a path segment.
//
// Specifically, it consists of the following character sets:
// - CONTROLS: 0x00-0x1F, 0x7F (control characters)
// - SPACE: 0x20
// - `"%"`: 0x25 (the percent sign itself needs to be escaped)
// - `"#"`: 0x23 (fragment separator)
// - `"<"`: 0x3C
// - `">"`: 0x3E
// - `"`: 0x22 (double quote)
// - "`{`: 0x7B
// - "`}`: 0x7D
// - "`|`: 0x7C
// - "`\`": 0x5C (backslash)
// - "`^`: 0x5E
// - "`` ` ": 0x60 (backtick)
//
// Considering the concepts of "reserved" and "unreserved" characters in RFC 2396,
// the '/' character, which has a special meaning in paths, is not escaped.
// Also, alphanumeric characters and some symbols (- _ . ~) are not escaped.
//
// In practice, non-ASCII characters such as Japanese are converted to a UTF-8 byte sequence,
// and then any byte exceeding 0x7F is escaped.
//
// This is adjusted to meet the requirements of the Trash specification (especially for multibyte characters),
// referencing the default sets of the `percent-encoding` crate.
// For example, there is a preset called `PATH_SEGMENT`, which also does not escape '/',
// so we aim for a behavior that is fundamentally similar to it.
//
// Here, in addition to `CONTROLS` and `SPACE`, we add a group of characters that require escaping in URI paths.
// This escapes characters that are allowed by the file system but are problematic as a URI.
const PATH_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ') // space
    .add(b'%')
    .add(b'#')
    .add(b'<')
    .add(b'>')
    .add(b'"')
    .add(b'{')
    .add(b'}')
    .add(b'|')
    .add(b'\\')
    .add(b'^')
    .add(b'`');

/// URL-escapes a file path according to the Trash specification.
pub fn trash_spec_url_encode(path: &str) -> String {
    // `utf8_percent_encode` converts non-ASCII characters into a UTF-8 byte sequence,
    // and then escapes bytes that are in `PATH_ENCODE_SET` or exceed 0x7F.
    // '/' is not included in `PATH_ENCODE_SET`, so it is not escaped.
    utf8_percent_encode(path, PATH_ENCODE_SET).to_string()
}

/// URL-decodes a file path according to the Trash specification.
pub fn trash_spec_url_decode(encoded_path: &str) -> Result<String, Utf8Error> {
    percent_decode_str(encoded_path)
        .decode_utf8()
        .map(|cow| cow.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trash_spec_url_encode() {
        struct TestCase<'a> {
            input: &'a str,
            expected: &'a str,
            description: &'a str,
        }

        let test_cases = vec![
            TestCase {
                input: "/home/user/Documents/テスト ファイル.txt",
                expected: "/home/user/Documents/%E3%83%86%E3%82%B9%E3%83%88%20%E3%83%95%E3%82%A1%E3%82%A4%E3%83%AB.txt",
                description: "Path with Japanese characters and spaces",
            },
            TestCase {
                input: "/path/to/my file with spaces.txt",
                expected: "/path/to/my%20file%20with%20spaces.txt",
                description: "Path with spaces",
            },
            TestCase {
                input: "/home/user/documents/report.pdf",
                expected: "/home/user/documents/report.pdf",
                description: "Path with no characters to escape",
            },
            TestCase {
                input: "/path/to/file%with%.txt",
                expected: "/path/to/file%25with%25.txt",
                description: "Path with percent signs",
            },
            TestCase {
                input: "/path/to/file#fragment.txt",
                expected: "/path/to/file%23fragment.txt",
                description: "Path with a hash/fragment symbol",
            },
            TestCase {
                input: r"/path/to/a\b/c<d>e{f}g|h^i`j.txt",
                expected: "/path/to/a%5Cb/c%3Cd%3Ee%7Bf%7Dg%7Ch%5Ei%60j.txt",
                description: "Path with various special characters",
            },
        ];

        for case in test_cases {
            assert_eq!(
                trash_spec_url_encode(case.input),
                case.expected,
                "Failed on: {}",
                case.description
            );
        }
    }

    #[test]
    fn test_trash_spec_url_decode() {
        // Test successful decoding
        assert_eq!(
            trash_spec_url_decode(
                "/home/user/Documents/%E3%83%86%E3%82%B9%E3%83%88%20%E3%83%95%E3%82%A1%E3%82%A4%E3%83%AB.txt"
            )
            .unwrap(),
            "/home/user/Documents/テスト ファイル.txt"
        );
        assert_eq!(
            trash_spec_url_decode("/path/to/my%20file%20with%20spaces.txt").unwrap(),
            "/path/to/my file with spaces.txt"
        );
        assert_eq!(
            trash_spec_url_decode("/path/to/file%25with%25.txt").unwrap(),
            "/path/to/file%with%.txt"
        );
        assert_eq!(
            trash_spec_url_decode("/home/user/documents/report.pdf").unwrap(),
            "/home/user/documents/report.pdf"
        );

        // Test that invalid percent-encoding sequences are passed through without error,
        // as this is the behavior of the `percent-encoding` crate.
        assert_eq!(
            trash_spec_url_decode("/path/to/file%GG.txt").unwrap(),
            "/path/to/file%GG.txt"
        );

        // Test invalid UTF-8 sequence
        let invalid_utf8 = trash_spec_url_decode("/path/to/%C3%28.txt");
        assert!(invalid_utf8.is_err(), "Should fail on invalid UTF-8 sequence");
    }
}
