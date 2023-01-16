#![feature(fs_try_exists)]

mod version;

use std::{
    fs::{File, OpenOptions},
    io::{copy, BufReader},
    path::PathBuf,
    slice::Iter,
};

use anyhow::{anyhow, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use zip::read::read_zipfile_from_stream;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    os: Option<String>,

    #[arg()]
    chromium_version: String,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err:?}");
    }
}

fn run() -> Result<()> {
    let args = Args::parse();
    let os = args.os.as_deref().unwrap_or(std::env::consts::OS);
    let os_prefix = get_os_prefix(os)?;

    // history.json 包含了 base_position 和版本号。
    let history_list = get_history_list(os)?;
    // builds 包含了所有可下载的 position 信息。
    let builds = get_build_list(os)?;

    // 用户提供的版本号，可能是一个主版本号，所以，可能匹配出很多个具体的版本。
    let matched_history_list = find_history(history_list.iter(), &args.chromium_version);
    // 从这些具体的版本中，找到一个具有 base_position 的版本。
    let mut matched_position = None;
    for history in matched_history_list {
        let deps = fetch_deps(&history.version)?;
        if let Some(pos) = deps.chromium_base_position {
            match pos.parse::<usize>() {
                Ok(pos) => {
                    if let Some((prefix, revision)) = find_builds(builds.iter(), pos, os_prefix) {
                        matched_position = Some((prefix, revision, deps.chromium_version));
                        break;
                    } else {
                        println!("==> no build found for rev: {pos}");
                    }
                }
                Err(err) => println!(
                    "==> chromium {}: parse base_position error: {:?}",
                    deps.chromium_version, err
                ),
            }
        } else {
            println!(
                "==> chromium {}: no chromium_base_position.",
                deps.chromium_version
            );
        }
    }

    let (found_prefix, _, found_chromium_ver) =
        matched_position.ok_or_else(|| anyhow!("未能根据版本号找到 position。"))?;

    // 根据 prefix 找到该版本文件列表，以及 chrome-win.zip 文件信息。
    let build_files = fetch_build_detail(found_prefix)?;
    let zip_file = ["chrome-win.zip", "chrome-win32.zip", "chrome-mac.zip"]
        .into_iter()
        .find_map(|f| build_files.iter().find(|file| file.name.ends_with(f)))
        .ok_or_else(|| {
            anyhow!(
                "在版本 {} 中，未找到 chrome-win.zip/chrome-win32-zip/chrome-mac.zip。",
                found_prefix
            )
        })?;

    // 开始下载压缩文件。
    println!("==> downloading {}", zip_file.media_link);
    let mut win_zip_response = reqwest::blocking::get(&zip_file.media_link)?;

    // 先保存到临时目录里面，待解压的时候，找到里面的版本信息，再重命名一下文件夹。
    let base_path = std::env::current_dir()?.join(format!("chromium-{found_chromium_ver}"));
    std::fs::create_dir_all(&base_path)?;

    // 执行 zip 解压过程，并去除压缩包的根目录。
    let mut prefix = String::new();
    loop {
        let mut zip = match read_zipfile_from_stream(&mut win_zip_response) {
            Ok(Some(zip)) => zip,
            Ok(None) => break,
            Err(err) => return Err(anyhow!("读取压缩文件出错: {:?}", err)),
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

fn get_os_prefix(os: &str) -> Result<&'static str> {
    match os {
        "windows" => Ok("Win_x64"),
        "macos" => Ok("Mac"),
        _ => Err(anyhow!("不支持的操作系统：{}", os)),
    }
}

fn get_build_list(os: &str) -> Result<Vec<String>> {
    let builds_json_path = get_cached_file_path(&format!("builds-{os}.json"))?;
    if std::fs::try_exists(&builds_json_path).unwrap_or_default() {
        println!("==> using cached builds.");
        Ok(serde_json::from_reader(BufReader::new(File::open(
            &builds_json_path,
        )?))?)
    } else {
        println!("==> retrieving builds ...");
        let pages = ChromiumBuildsPage::new(os)?;
        let mut unwrapped_page_list = Vec::new();
        for page in pages {
            unwrapped_page_list.push(page?);
        }
        let builds: Vec<String> = unwrapped_page_list.into_iter().flatten().collect();
        std::fs::write(&builds_json_path, serde_json::to_string(&builds)?)?;
        Ok(builds)
    }
}

fn get_history_list(os: &str) -> Result<Vec<ChromiumHistoryInfo>> {
    let history_json_path = get_cached_file_path(&format!("history-{os}.json"))?;
    if std::fs::try_exists(&history_json_path).unwrap_or_default() {
        println!("==> using cached history.");
        Ok(serde_json::from_reader(BufReader::new(File::open(
            &history_json_path,
        )?))?)
    } else {
        println!("==> retrieving history.json ...");
        let os_arg = match os {
            "windows" => "win64",
            "macos" => "mac",
            _ => return Err(anyhow!("不支持的操作系统：{}", os)),
        };
        let url = format!("https://omahaproxy.appspot.com/history.json?os={os_arg}&channel=stable");
        let response = reqwest::blocking::get(url)?;
        let history_list: Vec<ChromiumHistoryInfo> = serde_json::from_reader(response)?;
        std::fs::write(&history_json_path, serde_json::to_string(&history_list)?)?;
        Ok(history_list)
    }
}

fn get_cached_file_path(file: &str) -> Result<PathBuf> {
    let mut path = PathBuf::new();
    path.push(std::env::var("LOCALAPPDATA").or_else(|_| std::env::var("HOME"))?);
    path.push("fetchchromium");
    if !path.exists() {
        std::fs::create_dir_all(&path)?;
    }
    path.push(file);
    Ok(path)
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

fn fetch_deps(version: &str) -> Result<ChromiumDepsInfo> {
    let url = format!("https://omahaproxy.appspot.com/deps.json?version={version}");
    println!("==> fetching deps {url} ...");
    let response = reqwest::blocking::get(url)?;
    Ok(serde_json::from_reader(response)?)
}

fn find_history<'a>(
    history_list: Iter<'a, ChromiumHistoryInfo>,
    version: &str,
) -> Vec<&'a ChromiumHistoryInfo> {
    let ver_len = version.len();
    history_list
        .filter(|info| {
            info.version == version
                || (info.version.chars().nth(ver_len) == Some('.')
                    && info.version.starts_with(version))
        })
        .collect()
}

fn find_builds<'a>(
    build_list: Iter<'a, String>,
    find_pos: usize,
    os_prefix: &str,
) -> Option<(&'a String, usize)> {
    let mut list: Vec<_> = build_list
        .filter_map(|build| {
            let split: Vec<_> = build.split('/').collect();
            match split.as_slice() {
                &[prefix, rev, empty] if prefix == os_prefix && empty.is_empty() => {
                    rev.parse::<usize>().ok().map(|rev| (build, rev))
                }
                _ => None,
            }
        })
        .collect();
    list.sort_by(|a, b| a.1.cmp(&b.1));
    list.into_iter()
        .find(|build| build.1 >= find_pos)
        .filter(|build| (build.1 - find_pos <= 120))
}

pub(crate) struct ChromiumBuildsPage {
    prefix: &'static str,
    next_page_token: Option<String>,
    done: bool,
}

impl ChromiumBuildsPage {
    pub fn new(os: &str) -> Result<Self> {
        Ok(Self {
            next_page_token: None,
            done: false,
            prefix: get_os_prefix(os)?,
        })
    }
}

impl Iterator for ChromiumBuildsPage {
    type Item = Result<Vec<String>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            None
        } else {
            let next_page_token = self
                .next_page_token
                .as_ref()
                .map(|t| format!("&pageToken={t}"))
                .unwrap_or_default();
            let url = format!("https://www.googleapis.com/storage/v1/b/chromium-browser-snapshots/o?delimiter=/&prefix={}/&fields=items(kind,mediaLink,metadata,name,size,updated),kind,prefixes,nextPageToken{}", self.prefix, next_page_token);

            let prefixes = reqwest::blocking::get(&url)
                .map_err(|err| anyhow!("请求 {} 时出错：{:?}", url, err))
                .and_then(|response| {
                    let page: ChromiumBuildPage = serde_json::from_reader(response)?;
                    self.next_page_token = page.next_page_token;
                    self.done = self.next_page_token.is_none();
                    Ok(page.prefixes)
                });

            prefixes
                .map(|p| if p.is_empty() { None } else { Some(Ok(p)) })
                .unwrap_or_else(|e| Some(Err(e)))
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChromiumBuildPage {
    kind: String,
    next_page_token: Option<String>,
    #[serde(default)]
    prefixes: Vec<String>,
    #[serde(default)]
    items: Vec<GoogleApiStorageObject>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GoogleApiStorageObject {
    kind: String,
    media_link: String,
    name: String,
    size: String,
    updated: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ChromiumReleaseInfo {
    channel: String,
    chromium_main_branch_position: Option<usize>,
    milestone: usize,
    platform: String,
    previous_version: String,
    time: usize,
    version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ChromiumHistoryInfo {
    channel: String,
    os: String,
    timestamp: String,
    version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ChromiumDepsInfo {
    chromium_base_commit: Option<String>,
    chromium_base_position: Option<String>,
    chromium_branch: Option<String>,
    chromium_commit: String,
    chromium_version: String,
    skia_commit: String,
    v8_commit: String,
    v8_position: String,
    v8_version: String,
}
