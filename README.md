# truncanator

Fork of the [trunc_filenames](https://github.com/ssokolow/trunc_filenames) program by [ssokolow](https://github.com/ssokolow) that aims to extend the code and add new features.

**Full disclosure:** the code written to extend this program is AI-generated using DeepSeek-R1. Though I am dogfooding this same code, __you use this at your own risk__.

## Synopsis
```
$ ./trunc_filenames --help
Rename files and directories to fit length limits.

Preserve secondary extensions up to N characters (default: 6) using --secondary-ext-len. Set to 0 to disable extension preservation.

Usage: trunc_filenames [OPTIONS] [PATH]...

Arguments:
  [PATH]...  Paths to rename (recursively, if directories)

Options:
      --max-len <MAX_LEN>        Length to truncate to. (Default chosen for rclone name encryption) [default: 140]
  -n, --dry-run                  Don't actually rename files. Just print
  -s, --secondary-ext-len <LEN>  Maximum length to preserve for secondary extensions (e.g. 3 for ".tar" in ".tar.gz"). Set to 0 to disable [default: 6]
  -w, --word-boundaries          Respect word boundaries when truncating
  -h, --help                     Print help
  -V, --version                  Print version
```

## Current shortcomings

*Note: this was initially written by the original author*

1. Requires a POSIX platform because I didn't want to do something hacky and
   then forget, so I only bothered to implement "the length we care about is the
   _encoded_ length" based on `std::os::unix::ffi::OsStrExt`.
2. ~~Doesn't preserve secondary extensions like the `.tar` in `.tar.gz`~~ Fixed in Truncanator
3. If the file/directory name already contains bytes that aren't valid UTF-8, it
   won't bother to ensure that the truncation falls on the boundary between
   valid UTF-8 code points.

All of these are because it was a quick itch-scratch I would normally write in
Python and not even revision-control or upload, it works well enough for the
problem it was meant to solve, and solving any of those would have a
significantly lower return on investment.
