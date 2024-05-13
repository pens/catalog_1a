# imlib

This is a wrapper program for `exiftool` to help manage my photo library programmatically.

## Maintaining a Library

To clean up an existing catalog, run:
```
imlib clean -l /path/to/library
```

After the first run, only `imlib clean` needs to be called.

## Importing Photos & Videos

To import into an existing catalog, run:
```
imlib import /path/to/imported/items
```