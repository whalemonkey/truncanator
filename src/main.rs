//! Simple tool to rename files and directories to fit length limits.
//!
//! By default, preserves secondary extensions up to 6 characters each (e.g., .tar in .tar.gz).
//! Use --secondary-ext-len=0 to disable extension preservation.

use std::borrow::Cow;
use std::error::Error;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use clap::{
    builder::styling::{AnsiColor, Styles},
    Parser,
};
use walkdir::WalkDir;

fn styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Yellow.on_default())
        .usage(AnsiColor::Yellow.on_default())
        .literal(AnsiColor::Green.on_default())
        .placeholder(AnsiColor::Green.on_default())
}

/// Command-line argument schema
#[derive(Debug, Parser)]
#[command(version, about = "Rename files and directories to fit length limits.\n\nBy default, secondary extensions are preserved up to 6 characters; allowable length is adjustable using the -s argument.\n\nSet \"-s 0\" to disable secondary extension preservation.", long_about = None, styles = styles())]
struct CliArgs {
    /// Paths to rename (recursively, if directories)
    #[arg(required = true)]
    path: Vec<PathBuf>,

    /// Length to truncate to. (Default chosen for rclone name encryption)
    #[arg(long, default_value_t = 140)]
    max_len: usize,

    /// Don't actually rename files. Just print.
    #[arg(short = 'n', long, action, default_value_t = false)]
    dry_run: bool,

    /// Maximum length to preserve for secondary extensions
    /// (e.g. 3 for ".tar" in ".tar.gz").
    /// Set to 0 to disable.
    #[arg(short = 's', long, default_value_t = 6, value_name = "LEN")]
    secondary_ext_len: usize,

    /// Respect word boundaries when truncating
    #[arg(short = 'w', long, action, default_value_t = false)]
    word_boundaries: bool,
}

fn split_stem_ext(name: &str) -> (&str, Option<&str>) {
    if let Some((stem, ext)) = name.rsplit_once('.') {
        // Only consider it an extension if it's at the end and has no slashes
        if !ext.contains('/') && !ext.contains('\\') {
            (stem, Some(ext))
        } else {
            (name, None)
        }
    } else {
        (name, None)
    }
}

fn split_rstem_ext(name: &str, secondary_ext_len: usize) -> (String, Option<String>, Option<String>) {
    let (stem, primary_ext) = split_stem_ext(name);
    
    let primary_ext = primary_ext.map(|s| s.to_string());
    
    if secondary_ext_len == 0 {
        return (stem.to_string(), None, primary_ext);
    }
    
    // Check for valid secondary extension
    if let Some((r_stem, se)) = stem.rsplit_once('.') {
        if se.len() <= secondary_ext_len {
            return (r_stem.to_string(), Some(se.to_string()), primary_ext);
        }
    }
    
    (stem.to_string(), None, primary_ext)
}

/// Figure out the new name when truncating a path
///
/// **NOTE:** Handling of non-UTF8-able path is currently hacky
fn trunc_path(
    path: &Path,
    max_len: usize,
    secondary_ext_len: usize,
    word_boundaries: bool,
) -> Result<Cow<'_, Path>, Box<dyn Error>> {
    let is_dir = path.is_dir();
    let fname = match path.file_name() {
        Some(os_str) => os_str,
        None => return Ok(Cow::from(path)),
    };

    // Handle directories first with simpler truncation
    if is_dir {
        let name_str = fname.to_string_lossy();
        let (stem, _) = split_stem_ext(&name_str);
        
        let stem_bytes = stem.as_bytes();
        let max_stem_bytes = max_len;
        let mut truncated_bytes = &stem_bytes[..stem_bytes.len().min(max_stem_bytes)];

        // Preserve UTF-8 validity
        while !std::str::from_utf8(truncated_bytes).is_ok() {
            truncated_bytes = &truncated_bytes[..truncated_bytes.len()-1];
        }

        let mut truncated = String::from_utf8(truncated_bytes.to_vec())
            .unwrap_or_else(|_| String::new());


        // Preserve whole words where possible
        if word_boundaries {
            if let Some(last_space) = truncated.rfind(' ') {
                let space_bytes = truncated[..last_space].as_bytes().len();
                if space_bytes > max_stem_bytes.saturating_sub(10) {
                    truncated.truncate(last_space);
                }
            }
        }

        let parent = path.parent().unwrap_or_else(|| Path::new(""));
        let new_path = parent.join(truncated);
        return Ok(Cow::from(new_path));
    }

    // POSIX-specific but semantically correct. If I ever port this to Windows, I'll need to figure
    // out what RClone considers the length limit to be relative to anyway.
    let raw = fname.as_bytes();

    // Just return if it's already short enough
    let raw_trunc = if let Some(trunc) = raw.get(..max_len) {
        if raw.len() < max_len {
            return Ok(Cow::from(path));
        }
        trunc
    } else {
        return Ok(Cow::from(path));
    };

    if secondary_ext_len > 0 {
        if let Ok(fname_str) = std::str::from_utf8(raw) {
            // Split into main part and main extension
            let (main_part, main_ext) = match fname_str.rsplit_once('.') {
                Some((mp, me)) => (mp, me),
                None => (fname_str, ""),
            };

            // Check for valid secondary extension
            let (stem, secondary_ext) = match main_part.rsplit_once('.') {
                Some((s, se)) if se.len() <= secondary_ext_len => (s, Some(se)),
                _ => (main_part, None),
            };

            // Calculate total length needed for extensions in BYTES
            let ext_bytes = main_ext.as_bytes().len() + 1 +  // main extension + dot
                secondary_ext.map(|se| se.as_bytes().len() + 1).unwrap_or(0); // secondary extension + dot

            // Calculate available space for stem
            let max_stem_bytes = max_len.saturating_sub(ext_bytes);

            // Truncate stem from right without splitting words
            let stem_bytes = stem.as_bytes();
            let mut truncated_bytes = &stem_bytes[..stem_bytes.len().min(max_stem_bytes)];

            // Preserve UTF-8 validity
            while !std::str::from_utf8(truncated_bytes).is_ok() {
                truncated_bytes = &truncated_bytes[..truncated_bytes.len()-1];
            }

            let mut truncated_stem = String::from_utf8(truncated_bytes.to_vec())
                .unwrap_or_else(|_| String::new());
            
            // Preserve whole words where possible
            if word_boundaries {
                if let Some(last_space) = truncated_stem.rfind(' ') {
                    let space_bytes = truncated_stem[..last_space].as_bytes().len();
                    if space_bytes > max_stem_bytes.saturating_sub(10) {
                        truncated_stem.truncate(last_space);
                    }
                }
            }

            // Build new filename
            let mut new_fname = String::with_capacity(max_len);
            new_fname.push_str(&truncated_stem);

            if let Some(se) = secondary_ext {
                new_fname.push('.');
                new_fname.push_str(se);
            }
            new_fname.push('.');
            new_fname.push_str(main_ext);

            let parent = path.parent().unwrap_or_else(|| Path::new(""));
            let new_path = parent.join(new_fname);
            return Ok(Cow::from(new_path));
        }
    }

    let new_fname_len = if std::str::from_utf8(raw).is_ok() {
        // if it's UTF-8-able, then truncate and let the UTF-8 parser figure out where to split
        match std::str::from_utf8(raw_trunc) {
            Ok(_) => raw_trunc.len(),
            Err(e) => e.valid_up_to(),
        }
    } else {
        // For now, just let stuff that's already invalid UTF-8 end in a chopped code point
        //
        // TODO: Implement properly
        raw_trunc.len()
    };

    let path_raw = path.as_os_str().as_bytes();
    let mut new_len = path_raw.len() - (raw.len() - new_fname_len);
    if let Some(ext) = path.extension() {
        new_len = new_len.saturating_sub(ext.len()).saturating_sub(1); // for the dot
    }

    let new_result = path.as_os_str().as_bytes().get(..new_len).expect("truncate within len");

    let mut new_path = PathBuf::from(OsStr::from_bytes(new_result));
    if let Some(ext) = path.extension() {
        new_path.set_extension(ext);
    }
    Ok(Cow::from(new_path))
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = CliArgs::parse();

    for root in args.path {
        let mut file_groups = std::collections::HashMap::new();

        // First pass: Collect files by RStem and parent directory
        for entry in WalkDir::new(&root).contents_first(true) {
            // Convert path to owned PathBuf immediately to avoid lifetime issues
            let path = entry.as_ref()
                .map(|e| e.path().to_path_buf())
                .unwrap_or_else(|_| PathBuf::new());

            if path.is_dir() {
                continue;
            }

            let parent = path.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
            let fname = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string(); // Convert to owned String early

            // Split into components using the rules
            let (r_stem, secondary_ext, primary_ext) = split_rstem_ext(&fname, args.secondary_ext_len);

            file_groups.entry((parent, r_stem))
                .or_insert_with(Vec::new)
                .push((path, secondary_ext, primary_ext));
        }

        // Second pass: Process RStem groups
        for ((parent_dir, r_stem), files) in file_groups {
            // Calculate max stem length in BYTES for group
            let mut max_stem_bytes = usize::MAX;
            for (_, se, pe) in &files {
                // Calculate extension length in BYTES (including dots)
                let ext_bytes = pe.as_ref().map(|e| e.as_bytes().len() + 1).unwrap_or(0) +
                                se.as_ref().map(|e| e.as_bytes().len() + 1).unwrap_or(0);

                max_stem_bytes = max_stem_bytes.min(args.max_len.saturating_sub(ext_bytes));
            }

            // Truncate RStem to byte length (preserving UTF-8)
            let r_stem_bytes = r_stem.as_bytes();
            let mut truncated_bytes = &r_stem_bytes[..r_stem_bytes.len().min(max_stem_bytes)];

            // Find last valid UTF-8 boundary
            while !std::str::from_utf8(truncated_bytes).is_ok() {
                truncated_bytes = &truncated_bytes[..truncated_bytes.len()-1];
            }
            
            let mut truncated = String::from_utf8(truncated_bytes.to_vec())
                .unwrap_or_else(|_| String::new());

            // Update whole word preservation to use byte indices
            if args.word_boundaries {
                if let Some(last_space) = truncated.rfind(' ') {
                    let space_bytes = truncated[..last_space].as_bytes().len();
                    if space_bytes > max_stem_bytes.saturating_sub(10) {
                        truncated.truncate(last_space);
                    }
                }
            }

            // Process all files in group
            for (path, se, pe) in files {
                let mut new_name = truncated.clone();

                // Rebuild extensions
                if let Some(se) = se {
                    new_name.push('.');
                    new_name.push_str(&se);
                }
                if let Some(pe) = pe {
                    new_name.push('.');
                    new_name.push_str(&pe);
                }

                let new_path = parent_dir.join(&new_name);
                if new_path != path {
                    println!("Renaming: {:?} → {:?}", path.file_name().unwrap(), new_name);
                    if !args.dry_run {
                        std::fs::rename(&path, &new_path)?;
                    }
                }
            }
        }

        // Process directories separately after files
        for entry in WalkDir::new(root).contents_first(true) {
            let path = entry?.into_path();
            if path.is_dir() {
                let new_path = trunc_path(
                    &path, 
                    args.max_len, 
                    args.secondary_ext_len, 
                    args.word_boundaries
                )?;
                if new_path != path {
                    println!(
                        "Truncating directory: {:?} → {:?}",
                        path.file_name().unwrap(),
                        new_path.file_name().unwrap()
                    );
                    if !args.dry_run {
                        std::fs::rename(&path, &new_path)?;
                    }
                }
            }
        }
    }

    Ok(())
}

