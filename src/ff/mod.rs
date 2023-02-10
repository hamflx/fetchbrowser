use std::{env::current_dir, fs::create_dir_all, io::Cursor};

use anyhow::{anyhow, Result};
use compress_tools::{uncompress_archive, Ownership};

use crate::utils::find_sequence;

pub(crate) fn download_ff(version: &str) -> Result<()> {
    let url = format!(
        "https://ftp.mozilla.org/pub/firefox/releases/{version}/win64/zh-CN/Firefox%20Setup%20{version}.exe"
    );
    let exe_response = reqwest::blocking::get(url)?.bytes()?;
    let signature = b"7z\xbc\xaf\x27\x1c";
    let index_of_sig = find_sequence(exe_response.as_ref(), signature)
        .ok_or_else(|| anyhow!("No 7zip signature found"))?;

    let base_path = current_dir()?.join(format!("firefox-{version}"));
    create_dir_all(&base_path)?;

    let zip_content = exe_response.slice(index_of_sig..);
    uncompress_archive(Cursor::new(zip_content), &base_path, Ownership::Preserve)?;

    Ok(())
}
