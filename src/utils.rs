use std::path::PathBuf;

use anyhow::Result;

pub(crate) fn get_cached_file_path(file: &str) -> Result<PathBuf> {
    let mut path = PathBuf::new();
    path.push(std::env::var("LOCALAPPDATA").or_else(|_| std::env::var("HOME"))?);
    path.push("fetchchromium");
    if !path.exists() {
        std::fs::create_dir_all(&path)?;
    }
    path.push(file);
    Ok(path)
}

pub(crate) fn find_sequence<T: PartialEq>(haystack: &[T], needle: &[T]) -> Option<usize> {
    (0..haystack.len() - needle.len() + 1).find(|&i| haystack[i..i + needle.len()] == needle[..])
}
