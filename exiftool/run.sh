#!/usr/bin/env bash

cp test_image.jpg a.jpg
cp test_image.jpg b.jpg

# -tagsFromFiles.

exiftool -q -overwrite_original -Artist="Alice" -Copyright="Owner" a.jpg
exiftool -q -overwrite_original -Artist="Bob" b.jpg

# Only copies specified tags.
exiftool -q -overwrite_original -tagsFromFile a.jpg -Copyright b.jpg
exiftool -T -FileName -Artist b.jpg

# Copies all tags.
exiftool -q -overwrite_original -tagsFromFile a.jpg b.jpg
exiftool -T -FileName -Artist b.jpg

# -tagsFromFiles for rename.

exiftool -q -overwrite_original -Artist="Alice" a.jpg
exiftool -q -overwrite_original -Artist="Bob" b.jpg
exiftool -q -overwrite_original -DateTimeOriginal="1970:1:1 12:00:00" a.jpg
exiftool -q -overwrite_original -DateTimeOriginal="2024:5:18 5:03:00" b.jpg

# Need to specify DateTimeOriginal tag or this will copy *all*.
exiftool -q -overwrite_original -tagsFromFile a.jpg -DateTimeOriginal -d "%y%m%d_%H%M%S" b.jpg
exiftool -T -FileName -Artist -DateTimeOriginal b.jpg

# Renaming.

exiftool -q -overwrite_original -Artist="Alice" a.jpg
exiftool -q -overwrite_original -Artist="Bob" b.jpg
exiftool -q -overwrite_original -DateTimeOriginal="1970:1:1 12:00:00" a.jpg
exiftool -q -overwrite_original -DateTimeOriginal="2024:5:18 5:03:00" b.jpg

# If redirecting with <, only specified tags copied.
exiftool -q -overwrite_original -tagsFromFile a.jpg -d "%y%m%d_%H%M%S" "-TestName<DateTimeOriginal" b.jpg
exiftool -T -FileName -Artist b.jpg

# Get rename output, which can be compared to FileName.
exiftool -q -overwrite_original -tagsFromFile a.jpg -d "%y%m%d_%H%M%S" "-TestName<DateTimeOriginal" b.jpg
exiftool -T -FileName b.jpg

# Creating XMPs.

exiftool -q -overwrite_original -Artist="Alice" a.jpg
exiftool -q -overwrite_original -Artist="Bob" b.jpg
exiftool -q -overwrite_original -DateTimeOriginal="1970:1:1 12:00:00" a.jpg
exiftool -q -overwrite_original -DateTimeOriginal="2024:5:18 5:03:00" b.jpg

exiftool -v -o a.xmp a.jpg
exiftool -T -FileName -Artist -DateTimeOriginal a.jpg
exiftool -T -FileName -FileModifyDate -Artist -DateTimeOriginal a.xmp

rm a.jpg b.jpg a.xmp "-v"