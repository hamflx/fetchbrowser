use std::{fs::OpenOptions, io::copy, path::Path};

use anyhow::anyhow;
use zip::read::read_zipfile_from_stream;

use super::builds::GoogleApiStorageObject;

pub(crate) fn download_chromium_zip_file(
    zip_file: &GoogleApiStorageObject,
    base_path: &Path,
) -> std::result::Result<(), anyhow::Error> {
    // 开始下载压缩文件。
    println!("==> downloading {}", zip_file.media_link);
    let mut win_zip_response = reqwest::blocking::get(&zip_file.media_link)?;

    loop {
        let mut zip = match read_zipfile_from_stream(&mut win_zip_response) {
            Ok(Some(zip)) => zip,
            Ok(None) => break,
            Err(err) => return Err(anyhow!("读取压缩文件出错：{:?}", err)),
        };

        let zip_name = zip.name();
        println!("==> unzip: {zip_name}");

        if zip_name.starts_with("chrome-win/")
            || zip_name.starts_with("chrome-win32/")
            || zip_name.starts_with("chrome-mac/")
            || zip_name.starts_with("chrome-linux/")
        {
            let prefix_len = zip_name.find('/').unwrap() + 1;
            let file_path = base_path.join(&zip_name[prefix_len..]);
            if zip.is_dir() {
                std::fs::create_dir_all(&file_path).map_err(|err| {
                    anyhow!(
                        "创建目录 {} 时出错：{:?}",
                        file_path.to_str().unwrap_or_default(),
                        err
                    )
                })?;
            } else {
                if let Some(parent_dir) = file_path.parent() {
                    let _ = std::fs::create_dir_all(parent_dir);
                }
                copy(
                    &mut zip,
                    &mut OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .create(true)
                        .open(&file_path)
                        .map_err(|err| {
                            anyhow!(
                                "解压文件 {} 时出错：{:?}",
                                file_path.to_str().unwrap_or_default(),
                                err
                            )
                        })?,
                )
                .map_err(|err| {
                    anyhow!(
                        "解压文件 {} 时出错：{:?}",
                        file_path.to_str().unwrap_or_default(),
                        err
                    )
                })?;
            }
        } else {
            return Err(anyhow!("压缩包文件结构不正确。"));
        }
    }

    Ok(())
}
