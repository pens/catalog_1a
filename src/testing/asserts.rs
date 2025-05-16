// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! Extra asserts to make tests shorter / more readable.

#[macro_export]
macro_rules! assert_dir {
  ($dir:expr, [$($path:literal),* $(,)?]) => {{
    let actual = $dir.files_good();
    let expected = std::collections::HashSet::from([$($dir.get_path($path)),*]);

    assert!(
      actual == expected,
      "Directory contents do not match:\nActual:   {actual:#?}\nExpected: {expected:#?}"
    );
  }}
}

#[macro_export]
macro_rules! assert_err {
  ($res:expr, $msg:literal) => {{
    let Err(e) = $res else {
      panic!("Unexpected `Ok`.");
    };

    assert!(
      e.contains($msg),
      "Error message did not contain expected substring.\nActual:\n{e}\nExpected:\n{}",
      $msg
    );
  }};
}

#[macro_export]
macro_rules! assert_tag {
  // Tag should not be present.
  ($dir:expr, $file:literal, $tag:literal,None) => {{
    let actual = $crate::testing::read_tag(&$dir.root(), $file, None, $tag);

    if actual.is_some() {
      panic!(
        "{:?}:\nUnexpected `{}`:\n\tActual:   `{}`\n\tExpected: `None`",
        $dir.get_path($file),
        $tag,
        actual.as_ref().unwrap()
      );
    }
  }};

  ($dir:expr, $file:literal, $tag:literal, $expected:literal) => {{
    let tag_split = $tag.split(':').collect::<Vec<_>>();
    let tag_group = tag_split.get(1).map(|_| tag_split[0]);
    let tag_name = tag_split.get(1).unwrap_or(&tag_split[0]);

    let Some(actual) = $crate::testing::read_tag(&$dir.root(), $file, tag_group, tag_name) else {
      panic!(
        "{:?}:\nUnexpected `{}`:\n\tActual:   `None`\n\tExpected: `{}`",
        $dir.get_path($file),
        $tag,
        $expected
      );
    };

    assert!(
      actual == $expected,
      "{:?}:\nUnexpected `{}`:\n\tActual:   `{}`\n\tExpected: `{}`",
      $dir.get_path($file),
      $tag,
      actual,
      $expected,
    );
  }};
}

#[macro_export]
macro_rules! assert_trash {
  ($dir:expr, [$($path:literal),* $(,)?]) => {{
    let actual = $dir.files_trash();
    let expected = std::collections::HashSet::from([$($dir.get_trash($path)),*]);

    assert!(
      actual == expected,
      "Directory contents do not match:\nActual:   {actual:#?}\nExpected: {expected:#?}"
    );
  }}
}
