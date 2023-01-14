#![feature(fs_try_exists)]

use std::{
    fs::{File, OpenOptions},
    io::{copy, BufReader},
    path::PathBuf,
    slice::Iter,
};

use serde::{Deserialize, Serialize};
use zip::read::read_zipfile_from_stream;

fn main() {
    let ver = std::env::args().nth(1).unwrap();

    let builds = get_build_list();

    let history_list = get_history_list();
    let matched_history_list = find_history(history_list.iter(), &ver);

    let deps = matched_history_list
        .iter()
        .find_map(|history| fetch_deps(&history.version))
        .expect("No matched version");
    let (prefix, revision) =
        find_builds(builds.iter(), deps.chromium_base_position.parse().unwrap()).unwrap();
    println!("==> found nearest revision: {}", revision);

    let build_files = fetch_build_detail(prefix);
    let win_zip = build_files
        .iter()
        .find(|f| f.name.ends_with("chrome-win.zip"))
        .or_else(|| {
            build_files
                .iter()
                .find(|f| f.name.ends_with("chrome-win32.zip"))
        })
        .unwrap();

    println!("==> downloading {}", win_zip.media_link);
    let mut win_zip_response = reqwest::blocking::get(&win_zip.media_link).unwrap();

    let base_path = std::env::current_dir()
        .unwrap()
        .join(format!("tmp-chromium-{}", revision));
    std::fs::create_dir_all(&base_path).unwrap();

    let mut prefix = String::new();
    let mut version_list = Vec::new();
    loop {
        let mut zip = match read_zipfile_from_stream(&mut win_zip_response) {
            Ok(Some(zip)) => zip,
            Ok(None) => break,
            Err(err) => panic!("Error: {:?}", err),
        };

        let zip_name = zip.name();
        if prefix.is_empty() {
            if zip.is_dir() {
                prefix = zip.name().to_owned();
            } else {
                panic!("Invalid zip file");
            }
        } else if zip_name.starts_with(&prefix) {
            const MANIFEST_SUFFIX: &str = ".manifest";
            if zip_name.ends_with(MANIFEST_SUFFIX) {
                let manifest_name = zip_name
                    .rsplit_once('/')
                    .map(|(_, n)| n)
                    .unwrap_or(zip_name);
                let manifest_name =
                    manifest_name[..manifest_name.len() - MANIFEST_SUFFIX.len()].to_owned();
                if manifest_name
                    .split('.')
                    .into_iter()
                    .all(|part| part.parse::<usize>().is_ok())
                {
                    version_list.push(manifest_name);
                }
            }
            let file_path = base_path.join(&zip_name[prefix.len()..]);
            if zip.is_dir() {
                std::fs::create_dir_all(file_path).unwrap();
            } else {
                copy(
                    &mut zip,
                    &mut OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .create(true)
                        .open(file_path)
                        .unwrap(),
                )
                .unwrap();
            }
        } else {
            panic!("Invalid file name");
        }

        println!("==> unzip: {}", zip.name());
    }

    let version = find_latest_version(&version_list)
        .map(|(major, minor, branch, patch)| format!("{major}.{minor}.{branch}.{patch}"))
        .unwrap_or_else(|| revision.to_string());
    let dest_path = std::env::current_dir()
        .unwrap()
        .join(format!("chromium-{}", version));
    println!(
        "==> moving {} to {}",
        base_path.to_str().unwrap(),
        dest_path.to_str().unwrap()
    );
    std::fs::rename(base_path, dest_path).unwrap();
}

fn find_latest_version(version_list: &[String]) -> Option<(usize, usize, usize, usize)> {
    let mut latest_version = None;
    version_list.iter().for_each(|ver| {
        let split: Vec<_> = ver.split('.').collect();
        if let [major, minor, branch, patch] = split.as_slice() {
            let ver_tuple = (
                major.parse::<usize>().unwrap(),
                minor.parse::<usize>().unwrap(),
                branch.parse::<usize>().unwrap(),
                patch.parse::<usize>().unwrap(),
            );
            if let Some(ver) = &latest_version {
                if ver_tuple > *ver {
                    latest_version = Some(ver_tuple);
                }
            } else {
                latest_version = Some(ver_tuple);
            }
        }
    });
    latest_version
}

fn get_build_list() -> Vec<String> {
    let builds_json_path = get_cached_file_path("builds.json");
    if std::fs::try_exists(&builds_json_path).unwrap_or_default() {
        println!("==> using cached builds.");
        serde_json::from_reader(BufReader::new(File::open(&builds_json_path).unwrap())).unwrap()
    } else {
        println!("==> retrieving builds ...");
        let page = ChromiumBuildsPage::new();
        let builds: Vec<String> = page.flatten().collect();
        std::fs::write(&builds_json_path, serde_json::to_string(&builds).unwrap()).unwrap();
        builds
    }
}

fn get_history_list() -> Vec<ChromiumHistoryInfo> {
    let history_json_path = get_cached_file_path("history.json");
    if std::fs::try_exists(&history_json_path).unwrap_or_default() {
        println!("==> using cached history.");
        serde_json::from_reader(BufReader::new(File::open(&history_json_path).unwrap())).unwrap()
    } else {
        println!("==> retrieving history.json ...");
        let url = "https://omahaproxy.appspot.com/history.json?os=win&channel=stable";
        let response = reqwest::blocking::get(url).unwrap();
        let history_list: Vec<ChromiumHistoryInfo> = serde_json::from_reader(response).unwrap();
        std::fs::write(
            &history_json_path,
            serde_json::to_string(&history_list).unwrap(),
        )
        .unwrap();
        history_list
    }
}

fn get_cached_file_path(file: &str) -> PathBuf {
    let mut path = PathBuf::new();
    path.push(std::env::var("LOCALAPPDATA").unwrap());
    path.push("fetchchromium");
    if !path.exists() {
        std::fs::create_dir_all(&path).unwrap();
    }
    path.push(file);
    path
}

fn fetch_build_detail(prefix: &str) -> Vec<GoogleApiStorageObject> {
    let url = format!("https://www.googleapis.com/storage/v1/b/chromium-browser-snapshots/o?delimiter=/&prefix={prefix}&fields=items(kind,mediaLink,metadata,name,size,updated),kind,prefixes,nextPageToken");
    let response = reqwest::blocking::get(url).unwrap();
    let build_detail: ChromiumBuildPage = serde_json::from_reader(response).unwrap();
    build_detail.items
}

fn fetch_deps(version: &str) -> Option<ChromiumDepsInfo> {
    let url = format!("https://omahaproxy.appspot.com/deps.json?version={version}");
    let response = reqwest::blocking::get(url).unwrap();
    Some(serde_json::from_reader(response).unwrap())
}

fn find_history<'a>(
    history_list: Iter<'a, ChromiumHistoryInfo>,
    ver: &str,
) -> Vec<&'a ChromiumHistoryInfo> {
    let prefix = format!("{}.", ver);
    history_list
        .filter(|info| info.version.starts_with(&prefix))
        .collect()
}

fn find_builds(build_list: Iter<String>, find_pos: usize) -> Option<(&String, usize)> {
    let prefix = "Win_x64/";
    let mut list: Vec<_> = build_list
        .filter_map(|build| {
            if build.starts_with(prefix) {
                if let Ok(build_pos) = build[prefix.len()..build.len() - 1].parse::<usize>() {
                    Some((build, build_pos))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    list.sort_by(|a, b| a.1.cmp(&b.1));
    list.into_iter().rev().find(|build| build.1 <= find_pos)
}

pub(crate) struct ChromiumBuildsPage {
    next_page_token: Option<String>,
    done: bool,
}

impl ChromiumBuildsPage {
    pub fn new() -> Self {
        Self {
            next_page_token: None,
            done: false,
        }
    }
}

impl Iterator for ChromiumBuildsPage {
    type Item = Vec<String>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            None
        } else {
            let next_page_token = self
                .next_page_token
                .as_ref()
                .map(|t| format!("&pageToken={}", t))
                .unwrap_or_default();
            let url = format!("https://www.googleapis.com/storage/v1/b/chromium-browser-snapshots/o?delimiter=/&prefix=Win_x64/&fields=items(kind,mediaLink,metadata,name,size,updated),kind,prefixes,nextPageToken{}", next_page_token);
            let response = reqwest::blocking::get(url).unwrap();
            let page: ChromiumBuildPage = serde_json::from_reader(response).unwrap();
            self.next_page_token = page.next_page_token;
            self.done = self.next_page_token.is_none();
            if page.prefixes.is_empty() {
                None
            } else {
                Some(page.prefixes)
            }
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
    chromium_base_commit: String,
    chromium_base_position: String,
    chromium_branch: String,
    chromium_commit: String,
    chromium_version: String,
    skia_commit: String,
    v8_commit: String,
    v8_position: String,
    v8_version: String,
}
