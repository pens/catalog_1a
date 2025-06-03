# Catalog 1A

This is an abandoned attempt at building an automated system for maintaining my photo catalog.
While it was useful in deleting a number of unwanted files and renaming everything in a consistent format, I've found that now that my catalog is in a generally-good state that it's easier to just use [digiKam](https://www.digikam.org/).
On the rare occasion I need to do a one-off fix, a custom [ExifTool](https://exiftool.org/) script is faster (and likely safer).

## Unfinished Work

- There is a bug currently where metadata synchronization will overwrite subseconds.
  It is *not* safe to run.
- UTC renaming is not implemented.
- ExifTool's `-stay_open` is not implemented.
  This would likely be a huge performance win.
- The core `Organizer` type should be refactored in a more `data-oriented` approach (i.e. with each map "normalized").
- There is no runtime configuration of which passes to enable.

## Usage

### `org`: Catalog maintenance

```
c1a org [-c /path/to/catalog/] [-vv]
```

### `import`: Automatic import

```
c1a import /path/to/items/to/import/ [-vv]
```