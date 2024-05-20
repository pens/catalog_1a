//! Exiftool wrapper functions.
//!
//! Copyright 2024 Seth Pendergrass. See LICENSE.
use std::{ffi::OsStr, path::Path, process::Command};

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

/// Recursively gathers all metadata within root, optionally excluding the `exclude` (e.g. trash).
pub fn collect_metadata(root: &Path, exclude: Option<&Path>) -> Vec<u8> {
    let mut args = vec![
        "-Artist",
        "-ContentIdentifier",
        "-Copyright",
        "-CreateDate",
        "-DateTimeOriginal",
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

/// Copies metadata from `src` to `dst`.
pub fn copy_metadata(src: &Path, dst: &Path) {
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
}

/// Creates an XMP file for `path`, with all tags duplicated.
pub fn create_xmp(path: &Path) {
    run_exiftool(["-o", "%d%f.%e.xmp", path.to_str().unwrap()]);
}

/// Renames `path` according to `fmt`, optionally copying tags from `tag_src`.
pub fn rename_file(fmt: &str, path: &Path, tag_src: Option<&Path>) {
    let mut args = vec!["-d", "%Y/%m/%y%m%d_%H%M%S%%+c", fmt, path.to_str().unwrap()];

    if let Some(tag_src_path) = tag_src {
        let mut args2 = vec!["-tagsFromFile", tag_src_path.to_str().unwrap()];
        args2.append(&mut args);
        args = args2;
    }

    run_exiftool(args);
}
