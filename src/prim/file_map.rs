// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! Storage for homogeneous sets of files, allowing access by handle rather than
//! reference.

use std::{
  collections::HashMap,
  fmt::{self, Debug, Display, Formatter},
  hash::{Hash, Hasher},
  marker::PhantomData,
  ops::{Index, IndexMut},
  path::{Path, PathBuf},
};

/// Type-safe index into a `FileMap`.
/// Traits are explicitly implemented to avoid dependency on `T`.
pub struct Handle<T>(usize, PhantomData<T>);

impl<T> Clone for Handle<T> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<T> Copy for Handle<T> {}

impl<T> Debug for Handle<T> {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl<T> Default for Handle<T> {
  fn default() -> Self {
    Self(0, PhantomData)
  }
}

impl<T> Display for Handle<T> {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl<T> Eq for Handle<T> {}

impl<T> From<Handle<T>> for usize {
  fn from(h: Handle<T>) -> Self {
    h.0
  }
}

impl<T> From<usize> for Handle<T> {
  fn from(i: usize) -> Self {
    Self(i, PhantomData)
  }
}

impl<T> Hash for Handle<T> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.0.hash(state);
  }
}

impl<T> Index<Handle<T>> for Vec<Option<T>> {
  type Output = Option<T>;

  fn index(&self, index: Handle<T>) -> &Self::Output {
    &self[index.0]
  }
}

impl<T> IndexMut<Handle<T>> for Vec<Option<T>> {
  fn index_mut(&mut self, index: Handle<T>) -> &mut Self::Output {
    &mut self[index.0]
  }
}

impl<T> PartialEq for Handle<T> {
  fn eq(&self, other: &Self) -> bool {
    self.0 == other.0
  }
}

/// Holds a collection of files of the same type, each with a unique `Handle`.
pub struct FileMap<T> {
  data:           Vec<Option<T>>,
  path_to_handle: HashMap<PathBuf, Handle<T>>,
}

impl<T> FileMap<T> {
  /// Creates a new empty `FileMap`.
  pub fn new() -> Self {
    Self {
      data:           Vec::new(),
      path_to_handle: HashMap::new(),
    }
  }

  /// Finds the handle for `path`, if it exists.
  pub fn find(&self, path: impl AsRef<Path>) -> Option<Handle<T>> {
    self.path_to_handle.get(path.as_ref()).copied()
  }

  /// Returns a reference to the stored data for `handle`, allowing the caller
  /// to delete it. Note: The caller should *not* use this to overwrite the
  /// entry with a different file.
  pub fn get_entry_mut(&mut self, handle: Handle<T>) -> &mut Option<T> {
    &mut self.data[usize::from(handle)]
  }

  /// Adds a file to the map that was read from `path`.
  pub fn insert(&mut self, path: impl AsRef<Path>, data: T) {
    self.data.push(Some(data));
    self
      .path_to_handle
      .insert(path.as_ref().to_path_buf(), (self.data.len() - 1).into());
  }

  /// Iterates over all existing files.
  pub fn iter_data(&self) -> impl Iterator<Item = &T> {
    self.data.iter().flatten()
  }

  /// Iterates over all existing files, alongside their handles.
  pub fn iter_data_indexed(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
    self
      .data
      .iter()
      .enumerate()
      .filter_map(|(i, o)| o.as_ref().map(|v| (i.into(), v)))
  }

  /// Mutably iterates over all existing files.
  pub fn iter_data_mut(&mut self) -> impl Iterator<Item = &mut T> {
    self.data.iter_mut().flatten()
  }

  /// Mutably iterates over all existing files, alongside their handles.
  pub fn iter_data_mut_indexed(&mut self) -> impl Iterator<Item = (Handle<T>, &mut T)> {
    self
      .data
      .iter_mut()
      .enumerate()
      .filter_map(|(i, o)| o.as_mut().map(|v| (i.into(), v)))
  }

  /// Mutably iterates over all existing entries, allowing the caller to remove
  /// them. Note: The caller should *not* use this to overwrite the entry with
  /// a different file.
  pub fn iter_entries_mut(&mut self) -> impl Iterator<Item = &mut Option<T>> {
    self.data.iter_mut().filter(|o| o.is_some())
  }

  /// Mutably iterates over all existing entries, alongside their handles,
  /// allowing the caller to remove them. Note: The caller should *not* use
  /// this to overwrite the entry with a different file.
  pub fn iter_entries_mut_indexed(&mut self) -> impl Iterator<Item = (Handle<T>, &mut Option<T>)> {
    self
      .data
      .iter_mut()
      .enumerate()
      .filter(|(_, o)| o.is_some())
      .map(|(i, o)| (i.into(), o))
  }
}

impl<T> Default for FileMap<T> {
  fn default() -> Self {
    Self::new()
  }
}

impl<T> Index<Handle<T>> for FileMap<T> {
  type Output = T;

  fn index(&self, index: Handle<T>) -> &Self::Output {
    self.data[usize::from(index)].as_ref().unwrap()
  }
}

impl<T> IndexMut<Handle<T>> for FileMap<T> {
  fn index_mut(&mut self, index: Handle<T>) -> &mut Self::Output {
    self.data[usize::from(index)].as_mut().unwrap()
  }
}

#[cfg(test)]
mod test_find {
  use super::*;

  #[test]
  fn returns_expected_handles() {
    let mut map = FileMap::new();

    for (i, path) in ["image1.jpg", "image2.jpg", "image3.jpg"]
      .into_iter()
      .enumerate()
    {
      map.insert(path, i);
    }

    let handle1 = map.find("image1.jpg");
    let handle2 = map.find("image2.jpg");
    let handle3 = map.find("image3.jpg");

    assert!(handle1.is_some());
    assert!(handle2.is_some());
    assert!(handle3.is_some());
  }
}

#[cfg(test)]
mod test_iter {
  use super::*;

  #[test]
  fn skips_removed_items() {
    let mut map = FileMap::new();

    for (i, path) in ["image1.jpg", "image2.jpg", "image3.jpg"]
      .into_iter()
      .enumerate()
    {
      map.insert(path, i);
    }

    let handle1 = map.find("image1.jpg").unwrap();
    let handle2 = map.find("image2.jpg").unwrap();
    let handle3 = map.find("image3.jpg").unwrap();

    for entry in map.iter_entries_mut() {
      if entry.is_some_and(|i| i == 1) {
        entry.take();
      }
    }

    let handle_map = map.iter_data_mut_indexed().collect::<HashMap<_, _>>();

    assert_eq!(handle_map.len(), 2);
    assert_eq!(*handle_map[&handle1], 0);
    assert!(!handle_map.contains_key(&handle2));
    assert_eq!(*handle_map[&handle3], 2);
  }
}
