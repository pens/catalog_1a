/*
    Per-XMP-sidecar checks.

    Copyright 2023 Seth Pendergrass. See LICENSE.
*/
use super::Results;
use crate::file::File;

pub fn validate(file: &File, results: &mut Results) {
    //log::trace!("{}: Validating sidecar.", file.path.display());
    assert!(file.is_sidecar());

    link_sidecar_targets(file, results);
}

// This finds all files potentially being referenced by a sidecar, and save them to results.
fn link_sidecar_targets(file: &File, results: &mut Results) {
    log::trace!("Linking sidecar targets.");

    let mut has_target = false;

    // x.jpg.xmp -> x.jpg
    let expected_target = file.path.with_extension("");
    if expected_target.is_file() {
        log::trace!("\tFound {}", expected_target.display());
        has_target = true;
        results.add_sidecar_target(&file.path, &expected_target);
    }

    // x.xmp -> x.jpg
    /*
    TODO integrate Adobe-format XMP files for import from iOS

    let wildcard_target = file.path.with_extension("*");
    if let Ok(paths) = glob::glob(&wildcard_target.to_string_lossy()) {
        for entry in paths.flatten() {
            if file.path != entry {
                log::trace!("\tFound {}", entry.display());
                results.add_sidecar_target(&file.path, &entry);
            }
        }
    }
    */

    if !has_target {
        results.sidecars_without_targets.insert(file.path.clone());
    }
}
