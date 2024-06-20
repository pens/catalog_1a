//! Exiftool wrapper functions.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use std::path::PathBuf;
use std::{ffi::OsStr, path::Path, process::Command};

use regex::Regex;

/// Run exiftool with `args`, returning stdout.
/// Panics if exiftool fails.
fn run_exiftool<I, S>(args: I) -> Vec<u8>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = Command::new("exiftool");
    cmd.args(args);
    let output = cmd.output().unwrap();
    log::trace!(
        "exiftool output:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        output.status.success(),
        "exiftool failed with args: `{:#?}`. stderr: {}",
        cmd.get_args().collect::<Vec<&OsStr>>(),
        String::from_utf8_lossy(&output.stderr)
    );

    output.stdout
}

/// Given a byte stream from `exiftool`'s stdout, extracts the destination of a rename / move.
/// Expects the format: 'OLDNAME.jpg' --> 'NEWNAME.jpg'.
fn extract_destination(stdout: Vec<u8>) -> PathBuf {
    let stdout_string = String::from_utf8(stdout).unwrap();

    let re = Regex::new(r"'.+' --> '(.+)'").unwrap();
    let caps = re.captures(&stdout_string).unwrap();

    return PathBuf::from(caps.get(1).unwrap().as_str());
}

// TODO: Merge metadata functions into one.

/// Recursively gathers all metadata within root, optionally excluding the `exclude` (e.g. trash).
pub fn collect_metadata(root: &Path, exclude: Option<&Path>) -> Vec<u8> {
    let mut args = vec![
        "-Artist",
        "-ContentIdentifier",
        "-Copyright",
        "-CreateDate",
        "-DateTimeOriginal",
        "-FileModifyDate",
        "-FileType",
        "-FileTypeExtension",
        "-GPSAltitude",
        "-GPSAltitudeRef",
        "-GPSLatitude",
        "-GPSLatitudeRef",
        "-GPSLongitude",
        "-GPSLongitudeRef",
        "-Make",
        "-Model",
        "-json", // exiftool prefers JSON or XML over CSV.
        "-r",
        root.to_str().unwrap(),
    ];

    if let Some(exclude) = exclude {
        args.extend(["-i", exclude.to_str().unwrap()]);
    };

    run_exiftool(args)
}

pub fn get_metadata(path: &Path) -> Vec<u8> {
    let args = vec![
        "-Artist",
        "-ContentIdentifier",
        "-Copyright",
        "-CreateDate",
        "-DateTimeOriginal",
        "-FileModifyDate",
        "-FileType",
        "-FileTypeExtension",
        "-GPSAltitude",
        "-GPSAltitudeRef",
        "-GPSLatitude",
        "-GPSLatitudeRef",
        "-GPSLongitude",
        "-GPSLongitudeRef",
        "-Make",
        "-Model",
        "-json", // exiftool prefers JSON or XML over CSV.
        path.to_str().unwrap(),
    ];

    run_exiftool(args)
}

/// Copies metadata from `src` to `dst`.
pub fn copy_metadata(src: &Path, dst: &Path) -> Vec<u8> {
    run_exiftool([
        "-tagsFromFile",
        src.to_str().unwrap(),
        "-Artist",
        "-Copyright",
        "-CreateDate",
        "-DateTimeOriginal",
        "-GPSAltitude",
        "-GPSAltitudeRef",
        "-GPSLatitude",
        "-GPSLatitudeRef",
        "-GPSLongitude",
        "-GPSLongitudeRef",
        "-Make",
        "-Model",
        dst.to_str().unwrap(),
    ]);

    run_exiftool([
        "-Artist",
        "-ContentIdentifier",
        "-Copyright",
        "-CreateDate",
        "-DateTimeOriginal",
        "-FileModifyDate",
        "-FileType",
        "-FileTypeExtension",
        "-GPSAltitude",
        "-GPSAltitudeRef",
        "-GPSLatitude",
        "-GPSLatitudeRef",
        "-GPSLongitude",
        "-GPSLongitudeRef",
        "-Make",
        "-Model",
        dst.to_str().unwrap(),
    ])
}

/// Creates an XMP file for `path`, with all tags duplicated.
pub fn create_xmp(path: &Path) -> PathBuf {
    let stdout = run_exiftool(["-v", "-o", "%d%f.%e.xmp", path.to_str().unwrap()]);
    extract_destination(stdout)
}

/// Renames `path` according to `fmt`, optionally copying tags from `tag_src`.
pub fn rename_file(fmt: &str, path: &Path, tag_src: &Path) -> PathBuf {
    let mut args = vec!["-d", "%Y/%m/%y%m%d_%H%M%S%%+c", fmt, path.to_str().unwrap()];

    let mut args2 = vec!["-tagsFromFile", tag_src.to_str().unwrap()];
    args2.append(&mut args);
    args = args2;

    // TODO: check if file is already named correctly (TestName?)
    let stdout = run_exiftool(args);
    extract_destination(stdout)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_extract_destination() {
        let stdout = b"'old/path/name.jpg' --> 'new/path/name.jpg'";
        assert_eq!(
            extract_destination(stdout.to_vec()),
            PathBuf::from("new/path/name.jpg")
        );
    }
}
