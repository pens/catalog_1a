# imlib

This is a wrapper program for `exiftool` to help manage my photo library programmatically.

## Maintaining a Library

To clean up an existing catalog, run:
```
imlib clean
```
*On first run, you'll need to supply `-l /path/to/catalog` to set `imlib` up.*

## Importing Photos & Videos

To import into an existing catalog, run:
```
imlib import /path/to/imported/items
```