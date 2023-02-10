#![feature(fs_try_exists)]

mod builds;
mod ff;
mod history;
mod platform;
mod utils;
mod version;

use std::{fs::OpenOptions, io::copy, path::Path, str::FromStr};

use anyhow::{anyhow, Result};
use builds::{ChromiumBuilds, GoogleApiStorageObject};
use clap::Parser;
use ff::download_ff;
use history::ChromiumHistory;
use platform::{Arch, Os, Platform};
use zip::read::read_zipfile_from_stream;

use crate::builds::ChromiumBuildPage;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    os: Option<String>,

    #[arg()]
    browser_version: String,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err:?}");
    }
}

fn run() -> Result<()> {
    let args = Args::parse();
    if args.browser_version.starts_with("ff") {
        let ff_version = &args.browser_version[2..];
        download_ff(ff_version)?;
    } else {
        let os = Os::from_str(args.os.as_deref().unwrap_or(std::env::consts::OS))?;
        let x64platform = Platform::new(os, Arch::X86_64);
        if let Err(err) = download_chromium(&args.browser_version, x64platform) {
            // todo 这里不要无脑回退下载 x86，应该在版本找不到的时候才下载 x86 版本的。
            let x86platform = Platform::new(os, Arch::X86);
            if !x64platform.eq_impl(&x86platform) {
                println!("==> 下载 x64 版本出错，尝试 x86: {err}");
                download_chromium(&args.browser_version, x86platform)?;
            } else {
                return Err(err);
            }
        }
    }
    Ok(())
}

fn download_chromium(chromium_version: &str, platform: Platform) -> Result<()> {
    let os_prefix = platform.prefix();

    // history.json 包含了 base_position 和版本号。
    let history = ChromiumHistory::init(platform)?;
    // builds 包含了所有可下载的 position 信息。
    let builds = ChromiumBuilds::init(platform)?;

    // 用户提供的版本号，可能是一个主版本号，所以，可能匹配出很多个具体的版本。
    let matched_history_list = history.find(chromium_version);
    // 从这些具体的版本中，找到一个具有 base_position 的版本。
    let mut matched_position = None;
    for history in matched_history_list {
        let deps = history.deps()?;
        match deps.chromium_base_position {
            Some(pos) => match pos.parse::<usize>() {
                Ok(pos) => match builds.find(pos, os_prefix) {
                    Some(prefix) => {
                        matched_position = Some((prefix, deps.chromium_version));
                        break;
                    }
                    None => println!("==> no build found for rev: {pos}"),
                },
                Err(err) => println!(
                    "==> chromium {}: parse base_position error: {:?}",
                    deps.chromium_version, err
                ),
            },
            None => println!(
                "==> chromium {}: no chromium_base_position.",
                deps.chromium_version
            ),
        }
    }

    let (found_prefix, found_chromium_ver) =
        matched_position.ok_or_else(|| anyhow!("未能根据版本号找到 position。"))?;

    // 根据 prefix 找到该版本文件列表，以及 chrome-win.zip 文件信息。
    let build_files = fetch_build_detail(found_prefix)?;
    let zip_file = [
        "chrome-win.zip",
        "chrome-win32.zip",
        "chrome-mac.zip",
        "chrome-linux.zip",
    ]
    .into_iter()
    .find_map(|f| build_files.iter().find(|file| file.name.ends_with(f)))
    .ok_or_else(|| {
        anyhow!(
            "在版本 {} 中，未找到 chrome-win.zip/chrome-win32-zip/chrome-mac.zip。",
            found_prefix
        )
    })?;

    // 先保存到临时目录里面，待解压的时候，找到里面的版本信息，再重命名一下文件夹。
    let base_path = std::env::current_dir()?.join(format!("chromium-{found_chromium_ver}"));
    std::fs::create_dir_all(&base_path)?;
    download_zip_file(zip_file, &base_path)
}

fn download_zip_file(
    zip_file: &GoogleApiStorageObject,
    base_path: &Path,
) -> std::result::Result<(), anyhow::Error> {
    // 开始下载压缩文件。
    println!("==> downloading {}", zip_file.media_link);
    let mut win_zip_response = reqwest::blocking::get(&zip_file.media_link)?;

    // 执行 zip 解压过程，并去除压缩包的根目录。
    let mut prefix = String::new();
    loop {
        let mut zip = match read_zipfile_from_stream(&mut win_zip_response) {
            Ok(Some(zip)) => zip,
            Ok(None) => break,
            Err(err) => return Err(anyhow!("读取压缩文件出错：{:?}", err)),
        };

        let zip_name = zip.name();
        println!("==> unzip: {zip_name}");

        if prefix.is_empty() {
            if zip.is_dir() {
                prefix = zip.name().to_owned();
            } else {
                return Err(anyhow!("压缩包内目录结构不正确。"));
            }
        } else if zip_name.starts_with(&prefix) {
            let file_path = base_path.join(&zip_name[prefix.len()..]);
            if zip.is_dir() {
                std::fs::create_dir_all(&file_path).map_err(|err| {
                    anyhow!(
                        "创建目录 {} 时出错：{:?}",
                        file_path.to_str().unwrap_or_default(),
                        err
                    )
                })?;
            } else {
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

fn fetch_build_detail(prefix: &str) -> Result<Vec<GoogleApiStorageObject>> {
    let url = format!("https://www.googleapis.com/storage/v1/b/chromium-browser-snapshots/o?delimiter=/&prefix={prefix}&fields=items(kind,mediaLink,metadata,name,size,updated),kind,prefixes,nextPageToken");
    println!("==> fetching history {url} ...");
    let response = reqwest::blocking::get(url)?;
    let build_detail: ChromiumBuildPage = serde_json::from_reader(response)?;
    println!("==> files:");
    for file in &build_detail.items {
        println!("    {}", file.name);
    }
    Ok(build_detail.items)
}
