use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Test utilities for managing temporary files and directories
pub struct TestDir {
    pub dir: TempDir,
}

impl TestDir {
    pub fn new() -> Self {
        TestDir { dir: TempDir::new().expect("Failed to create temp directory") }
    }

    pub fn create_file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.dir.path().join(name);
        fs::write(&path, content).expect("Failed to write test file");
        path
    }

    pub fn create_dir(&self, name: &str) -> PathBuf {
        let path = self.dir.path().join(name);
        fs::create_dir(&path).expect("Failed to create test directory");
        path
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{process_directories, process_files, split_rstem_ext, trunc_path, CliArgs};
    use std::ffi::OsStr;

    /// Helper function to create test args
    fn test_args(
        path: PathBuf,
        max_len: usize,
        sec_ext_len: usize,
        word_boundaries: bool,
    ) -> CliArgs {
        CliArgs {
            path: vec![path],
            max_len,
            dry_run: false,
            secondary_ext_len: sec_ext_len,
            word_boundaries,
        }
    }

    #[test]
    fn test_primary_extension_preservation() {
        // Rule: Primary extension must NEVER be truncated
        let test_dir = TestDir::new();
        let test_cases = vec![
            ("verylongname.txt", 8, "ver.txt"),
            ("reallylongname.tar.gz", 10, "real.tar.gz"),
            ("超長い名前.txt", 12, "超長.txt"),
        ];

        for (input, max_len, expected) in test_cases {
            let path = test_dir.create_file(input, "content");
            let result = trunc_path(&path, max_len, 6, false).expect("Truncation failed");
            let result_str = result.to_str().unwrap();
            let result_ext = result_str.rsplit('.').next().unwrap();
            let expected_ext = expected.rsplit('.').next().unwrap();
            assert_eq!(result_ext, expected_ext, "Primary extension must be preserved");
        }
    }

    #[test]
    fn test_secondary_extension_rules() {
        // Rule: Secondary extension preservation based on length threshold
        let test_cases = vec![
            // (filename, sec_ext_len, expected_stem, expected_sec_ext, expected_pri_ext)
            ("file.tar.gz", 6, "file", Some("tar"), Some("gz")),
            ("file.tar.gz", 0, "file.tar", None, Some("gz")),
            ("file.toolong.txt", 6, "file.toolong", None, Some("txt")),
            ("file.sh.txt", 6, "file", Some("sh"), Some("txt")),
        ];

        for (input, sec_len, exp_stem, exp_sec, exp_pri) in test_cases {
            let input_os = OsStr::new(input);
            let (stem, sec_ext, pri_ext) = split_rstem_ext(input_os, sec_len);
            let stem_str = stem.to_string_lossy().into_owned();
            let sec_ext_str = sec_ext.as_ref().map(|e| e.to_string_lossy().into_owned());
            let pri_ext_str = pri_ext.as_ref().map(|e| e.to_string_lossy().into_owned());

            assert_eq!(stem_str, exp_stem);
            assert_eq!(sec_ext_str.as_deref(), exp_sec);
            assert_eq!(pri_ext_str.as_deref(), exp_pri);
        }
    }

    #[test]
    fn test_rstem_group_consistency() {
        // Rule: Files with same initial RStem in same directory must have same final RStem length
        let test_dir = TestDir::new();

        // Create files in same group with different extensions
        test_dir.create_file("document.txt", "content");
        test_dir.create_file("document.tar.gz", "content");
        test_dir.create_file("document.config", "content");

        let args = test_args(test_dir.path().to_path_buf(), 12, 6, false);

        process_files(&args).expect("File processing failed");

        let files: Vec<_> =
            fs::read_dir(test_dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();

        let rstems: Vec<_> = files.iter().map(|f| split_rstem_ext(f.as_ref(), 6).0).collect();

        let first_len = rstems[0].len();
        for rstem in rstems.iter().skip(1) {
            assert_eq!(rstem.len(), first_len, "All files in group must have same RStem length");
        }
    }

    #[test]
    fn test_word_boundary_truncation() {
        // Rule: Word boundary respect when -w flag is used
        let test_dir = TestDir::new();
        let test_cases = vec![
            // With word boundaries (-w flag), should truncate at last space before max_len - 10
            ("this is a long filename.txt", 15, "this is a.txt", true),
            // Without word boundaries (default), should truncate exactly at max_len
            ("no_word_boundaries_here.txt", 12, "no_word_.txt", false),
            // With word boundaries, should truncate at last space
            ("respect these words.txt", 14, "respect.txt", true),
        ];

        for (input, max_len, expected, word_boundaries) in test_cases {
            let path = test_dir.create_file(input, "content");
            let result = trunc_path(&path, max_len, 6, word_boundaries).expect("Truncation failed");
            let result_str = result.file_name().unwrap().to_str().unwrap().to_string();
            assert_eq!(
                result_str, expected,
                "Word boundary truncation failed for '{}' with max_len={}",
                input, max_len
            );
        }
    }

    #[test]
    fn test_utf8_boundaries() {
        // Rule: Truncation must occur at valid UTF-8 boundaries
        let test_dir = TestDir::new();
        let test_cases = vec![
            ("日本語.txt", 8, true), // Valid truncation
            ("🌟star.txt", 7, true), // Emoji handling
            ("αβγ.txt", 5, true),    // Greek letters
        ];

        for (input, max_len, should_be_valid) in test_cases {
            let path = test_dir.create_file(input, "content");
            let result = trunc_path(&path, max_len, 6, false).expect("Truncation failed");

            assert_eq!(result.to_str().is_some(), should_be_valid, "Result must be valid UTF-8");
        }
    }

    #[test]
    fn test_directory_truncation() {
        // Rule: Directories truncate independently, only from right side
        let test_dir = TestDir::new();
        let test_cases = vec![
            ("very_long_directory", 8, "very_lon"),
            ("日本語ディレクトリ", 12, "日本語デ"),
            ("spaces_in_name", 7, "spaces_"),
        ];

        for (dirname, max_len, expected) in test_cases {
            let dir_path = test_dir.create_dir(dirname);
            let args = test_args(dir_path.clone(), max_len, 6, false);

            process_directories(&args).expect("Directory processing failed");
            let new_path = dir_path.parent().unwrap().join(expected);
            assert!(new_path.exists(), "Directory should be truncated to '{}'", expected);
        }
    }

    #[test]
    fn test_skip_oversized_files() {
        // Rule: Skip files where extensions + minimum RStem exceed max_len
        let test_dir = TestDir::new();
        // Test case where extensions alone exceed max_len
        let filename = "test.tar.gz"; // .tar.gz is 7 bytes
        let path = test_dir.create_file(filename, "content");

        let args = test_args(path.clone(), 6, 6, false); // max_len less than extensions

        process_files(&args).expect("Processing should succeed");

        // Verify file was skipped (not modified)
        assert!(path.exists(), "Original file should remain unchanged");
        assert_eq!(
            path.file_name().unwrap().to_str().unwrap(),
            filename,
            "File should be skipped when extensions exceed max_len"
        );

        // Test case where minimum stem + extensions exceed max_len
        let filename2 = "doc.tar.gz"; // 3 + 7 = 10 bytes minimum
        let path2 = test_dir.create_file(filename2, "content");

        let args2 = test_args(path2.clone(), 8, 6, false); // max_len less than minimum possible

        process_files(&args2).expect("Processing should succeed");

        assert_eq!(
            path2.file_name().unwrap().to_str().unwrap(),
            filename2,
            "File should be skipped when minimum possible length exceeds max_len"
        );
    }
}
