# imlib

This is a wrapper program for `exiftool` to help manage my photo library programmatically.

## Future Work

- Check that `Events/*` tags are continuous / non-overlapping (is this possible?).
- Add check for timezones.
  - Warn when timezone doesn't match geotag.
- Support Google's Motion Photos.
- Add check that Artist and Copyright fields are filled out.
  - Add checks against my known cameras.
- See if `fast2` flag with `exiftool` would speed things up.