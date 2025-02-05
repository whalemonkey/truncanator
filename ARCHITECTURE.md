# Truncanator Architecture

## Overview

Truncanator is a Rust-based CLI utility designed to rename files and directories to fit specific length limits while preserving file extensions. It's particularly useful for systems with filename length restrictions, such as when using rclone's name encryption.

## Core Features

- Truncates filenames and directory names to a specified maximum length (default: 140 characters)
- Preserves primary and secondary file extensions (e.g., both extensions in `.tar.gz`)
- Supports word boundary-aware truncation (optional)
- Handles UTF-8 encoding correctly
- Provides dry-run capability for safe testing
- Processes files and directories recursively

## Architectural Design

### Component Structure

The tool is organized into several key components:

1. **CLI Interface** (`CliArgs` struct)
   - Handles command-line argument parsing using `clap`
   - Provides styled output for better user experience
   - Defines core configuration parameters:
     - Maximum length limit (--max-len)
     - Secondary extension length (-s, --secondary-ext-len)
     - Word boundary preservation (-w, --word-boundaries)
     - Dry-run mode (-n, --dry-run)

2. **Path Processing**
   - Split into two main operations:
     - File processing (`process_files`)
     - Directory processing (`process_directories`)
   - Uses depth-first, contents-first traversal for safe renaming

3. **Name Manipulation**
   - Extension handling (`split_stem_ext`, `split_rstem_ext`)
   - Path truncation (`trunc_path`)
   - UTF-8 validation and preservation

### Key Design Decisions

1. **Two-Pass File Processing**
   - First pass: Groups files by root stem and parent directory
   - Second pass: Processes groups to ensure consistent truncation
   - Rationale: Ensures related files maintain consistent naming

2. **Extension Preservation**
   - Preserves secondary extensions up to configurable length (default: 6 chars)
   - Can be disabled via CLI flag (--secondary-ext-len=0)
   - Rationale: Maintains file type identification and related file grouping

3. **UTF-8 Handling**
   - Careful boundary checking for UTF-8 validity
   - Falls back to byte-based truncation when necessary
   - Rationale: Ensures no corruption of Unicode characters

4. **Word Boundary Preservation**
   - Optional feature to prevent words from being cut mid-way
   - Uses space character as word boundary
   - Includes safety margin (10 bytes) to prevent excessive shortening
   - Rationale: Improves readability of truncated names

## Implementation Details

### Core Functions

1. `split_stem_ext`
   - Purpose: Splits filename into stem and extension
   - Handles path separator checks
   - Returns: `(&OsStr, Option<&OsStr>)`
   - Implementation: Uses byte-level operations for accurate path handling

2. `split_rstem_ext`
   - Purpose: Advanced splitting with secondary extension support
   - Parameters: Controls secondary extension length
   - Returns: `(OsString, Option<OsString>, Option<OsString>)`
   - Implementation: Builds on `split_stem_ext` for additional extension handling

3. `trunc_path`
   - Purpose: Main truncation logic
   - Handles both files and directories
   - Preserves UTF-8 validity
   - Returns: `Result<Cow<'_, Path>, Box<dyn Error>>`
   - Implementation: Separate logic paths for files and directories

### Processing Flow

1. **Directory Processing**
   ```rust
   process_directories
   └── trunc_path
       └── rename if needed
   ```

2. **File Processing**
   ```rust
   process_files
   ├── collect files by RStem
   ├── calculate_max_stem_bytes
   ├── truncate_stem
   └── build_new_name
   ```

### Error Handling

- Uses Rust's Result type for error propagation
- Provides informative warnings for skipped files
- Handles filesystem errors gracefully
- Implements proper error context through std::error::Error trait

## Security Considerations

### Filesystem Operations

1. **Current Implementation**
   - Uses standard filesystem operations through std::fs
   - Respects existing filesystem permissions
   - Maintains parent directory relationships
   - Performs atomic rename operations where possible

2. **Data Safety Features**
   - Dry-run mode for safe testing
   - Validates paths before operations
   - Preserves file extensions by default
   - Reports errors without partial changes

### Recommendations for Use

1. **Before Bulk Operations**
   - Run with --dry-run first
   - Backup important data
   - Verify sufficient permissions
   - Check available disk space

2. **During Operation**
   - Monitor operation output
   - Check for warning messages
   - Verify renamed files

## Known Limitations

1. **Current Implementation Constraints**
   - POSIX-specific path handling (noted in code comments)
   - Limited to local filesystem operations
   - Single-threaded processing
   - In-memory file grouping (may impact large directories)

2. **UTF-8 Handling**
   - May truncate at sub-optimal points for some Unicode sequences
   - Handles basic UTF-8 validation but not normalization
   - No special handling for bidirectional text
   - Limited support for complex Unicode edge cases

3. **Extension Handling**
   - Maximum two-level extension preservation
   - Fixed-length secondary extension limit
   - No special handling for hidden files
   - Extension length counts toward total length

## Usage Guidelines

### Basic Usage

```bash
trunc_filenames [OPTIONS] <PATH>...
```

The tool accepts one or more paths as arguments. For each provided path:
- If it's a directory, the tool recursively processes all files and subdirectories within it
- If it's a file, the tool processes just that file
- All paths are processed using the same set of options

### Key Options

- `--max-len <N>`: Maximum filename length (default: 140)
  - Applied to each individual filename or directory name
  - Includes any extensions in the length calculation
  
- `-s, --secondary-ext-len <LEN>`: Maximum secondary extension length (default: 6)
  - Controls preservation of extensions like "tar" in "file.tar.gz"
  - Only preserves extensions up to this length
  - Set to 0 to disable secondary extension preservation entirely
  
- `-w, --word-boundaries`: Respect word boundaries when truncating
  - Only breaks at space characters
  - Includes a 10-byte safety margin to prevent excessive shortening
  - Example: "very_long_filename" with max_len=8 becomes "very" instead of "very_lon"
  
- `-n, --dry-run`: Preview changes without renaming
  - Shows what would be renamed without making changes
  - Useful for verifying behavior before actual modification

### Processing Order

1. Files are processed before their containing directories
2. Within each directory:
   - Files with the same root stem are grouped and processed together
   - This ensures consistent truncation for related files
   - Example: "document.tar.gz" and "document.txt" maintain the same stem length

### Examples

1. Process a single directory recursively:
   ```bash
   trunc_filenames long_filename_directory/
   ```
   - Processes all files and subdirectories within long_filename_directory
   - Maintains directory structure while renaming

2. Process multiple specific paths:
   ```bash
   trunc_filenames path1/files/ path2/docs/ path3/long_filename.txt
   ```
   - Processes each path independently
   - Can mix files and directories

3. Preserve word boundaries with custom length:
   ```bash
   trunc_filenames -w --max-len 100 path/to/files/
   ```
   - Truncates at space characters
   - May truncate shorter than 100 to respect word boundaries
   - Maintains at least 90% of max_len (10-byte margin)

4. Disable secondary extension preservation:
   ```bash
   trunc_filenames --secondary-ext-len 0 path/to/files/
   ```
   - Only preserves the final extension
   - Example: "document.tar.gz" might become "doc.gz"

## Development Guidelines

### Adding New Features

1. **CLI Arguments**
   - Add to `CliArgs` struct with appropriate documentation
   - Include default values where sensible
   - Update help text as needed

2. **Name Processing**
   - Consider UTF-8 implications
   - Maintain extension preservation logic
   - Add appropriate error handling

3. **Testing Considerations**
   - Test with various filename patterns
   - Include UTF-8 edge cases
   - Verify extension handling
   - Test error conditions

### Code Style

- Follow Rust idioms and best practices
- Maintain comprehensive documentation
- Use meaningful variable names
- Include explanatory comments for complex logic

### Error Handling Best Practices

1. Use appropriate error types
2. Provide meaningful error messages
3. Handle edge cases explicitly
4. Include context in error reporting

## Performance Characteristics

### Current Implementation

1. **Memory Usage**
   - Groups files by root stem and parent directory
   - Keeps file groups in memory during processing
   - Uses references where possible
   - Minimizes string allocations

2. **File System Operations**
   - Single directory traversal pass
   - Processes directories contents-first
   - Performs renames only when needed
   - Groups related operations

3. **String Processing**
   - Byte-level operations for efficiency
   - UTF-8 validation on truncation
   - Minimal string conversions
   - Uses OsString for paths

### Performance Considerations

1. **Directory Size Impact**
   - Memory usage scales with number of related files
   - Processing time is linear with file count
   - Group operations may impact large directories

2. **Filename Characteristics**
   - UTF-8 validation adds overhead
   - Word boundary checks impact processing
   - Extension preservation requires additional parsing

Note: This documentation reflects the current implementation as of the source code review. Future versions may add additional features or modify existing behaviors.