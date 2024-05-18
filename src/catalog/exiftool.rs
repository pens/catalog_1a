use std::{ffi::OsStr, path::Path, process::Command};

/// Run exiftool with `args`, returning stdout.
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

pub fn collect_metadata(root: &Path, exclude: Option<&Path>) -> Vec<u8> {
    // TODO cleanup
    if let Some(exclude) = exclude {
        return run_exiftool([
            "-FileType",
            "-FileTypeExtension",
            "-ContentIdentifier",
            "-MediaGroupUUID",
            "-CreateDate",
            "-DateTimeOriginal",
            "-json", // exiftool prefers JSON or XML over CSV.
            "-r",
            root.to_str().unwrap(),
            "-i",
            exclude.to_str().unwrap(),
        ]);
    } else {
        run_exiftool([
            "-FileType",
            "-FileTypeExtension",
            "-ContentIdentifier",
            "-MediaGroupUUID",
            "-CreateDate",
            "-DateTimeOriginal",
            "-json", // exiftool prefers JSON or XML over CSV.
            "-r",
            root.to_str().unwrap(),
        ])
    }
}

pub fn copy_metadata(src: &Path, dst: &Path) {
    run_exiftool([
        "-tagsFromFile",
        src.to_str().unwrap(),
        "-CreateDate",
        "-DateTimeOriginal",
        "-Artist",
        "-Copyright",
        // https://exiftool.org/TagNames/GPS.html recommends all of the below
        "-GPSLatitude",
        "-GPSLatitudeRef",
        "-GPSLongitude",
        "-GPSLongitudeRef",
        "-GPSAltitude",
        "-GPSAltitudeRef",
        dst.to_str().unwrap(),
    ]);
}

pub fn create_xmp(path: &Path) {
    run_exiftool(["-o", "%d%f.%e.xmp", path.to_str().unwrap()]);
}

pub fn rename_file(fmt: &str, path: &Path, tag_src: Option<&Path>) {
    // TODO cleanup
    if let Some(tag_src_path) = tag_src {
        run_exiftool([
            "-tagsFromFile", // Note: This overwrites the file's tags with tag_src_path's.
            tag_src_path.to_str().unwrap(),
            "-d",
            "%Y/%m/%y%m%d_%H%M%S%%+c",
            fmt,
            path.to_str().unwrap(),
        ]);
    } else {
        run_exiftool([
            "-d",
            "%Y/%m/%y%m%d_%H%M%S%%+c",
            &fmt,
            path.to_str().unwrap(),
        ]);
    }
}