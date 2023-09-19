use crate::process;
use crate::scan;
use std::path::Path;

pub fn import(library_root: &Path, apply_changes: bool) {
    assert!(library_root.is_dir());
    log::info!("Importing from {}", library_root.display());

    let path = library_root.join("import");

    /*
    let scan_results = scan::scan_internal(&path);
    process::process(&scan_results, apply_changes);
    */
}
