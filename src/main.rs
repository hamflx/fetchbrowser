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
    let ver = std::env::args().skip(1).next().unwrap();

    let builds = get_build_list();

    let release_list = get_release_list();
    let release_info = find_release(release_list.iter(), &ver).unwrap();

    let (prefix, revision) =
        find_builds(builds.iter(), release_info.chromium_main_branch_position).unwrap();
    let build_files = fetch_build_detail(prefix);
    let win_zip = build_files
        .into_iter()
        .find(|f| f.name.ends_with("chrome-win.zip"))
        .unwrap();

    println!("==> downloading {}", win_zip.media_link);
    let mut win_zip_response = reqwest::blocking::get(win_zip.media_link).unwrap();

    let base_path = std::env::current_dir()
        .unwrap()
        .join(format!("chrome-{}", revision));
    std::fs::create_dir_all(&base_path).unwrap();

    let mut prefix = String::new();
    loop {
        let mut zip = match read_zipfile_from_stream(&mut win_zip_response) {
            Ok(Some(zip)) => zip,
            Ok(None) => break,
            Err(err) => panic!("Error: {:?}", err),
        };

        if prefix.is_empty() {
            if zip.is_dir() {
                prefix = zip.name().to_owned();
            } else {
                panic!("Invalid zip file");
            }
        } else if zip.name().starts_with(&prefix) {
            let file_path = base_path.join(&zip.name()[prefix.len()..]);
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

fn get_release_list() -> Vec<ChromiumReleaseInfo> {
    let releases_json_path = get_cached_file_path("releases.json");
    if std::fs::try_exists(&releases_json_path).unwrap_or_default() {
        println!("==> using cached releases.");
        serde_json::from_reader(std::fs::File::open(&releases_json_path).unwrap()).unwrap()
    } else {
        println!("==> retrieving releases ...");
        let url = "https://chromiumdash.appspot.com/fetch_releases";
        let response = reqwest::blocking::get(url).unwrap();
        let release_list: Vec<ChromiumReleaseInfo> = serde_json::from_reader(response).unwrap();
        std::fs::write(
            &releases_json_path,
            serde_json::to_string(&release_list).unwrap(),
        )
        .unwrap();
        release_list
    }
}

fn fetch_build_detail(prefix: &str) -> Vec<GoogleApiStorageObject> {
    let url = format!("https://www.googleapis.com/storage/v1/b/chromium-browser-snapshots/o?delimiter=/&prefix={prefix}&fields=items(kind,mediaLink,metadata,name,size,updated),kind,prefixes,nextPageToken");
    let response = reqwest::blocking::get(url).unwrap();
    let build_detail: ChromiumBuildPage = serde_json::from_reader(response).unwrap();
    build_detail.items
}

fn find_release<'a>(
    mut release_list: Iter<'a, ChromiumReleaseInfo>,
    ver: &str,
) -> Option<&'a ChromiumReleaseInfo> {
    let prefix = format!("{}.", ver);
    release_list.find(|info| info.version.starts_with(&prefix))
}

fn find_builds<'a>(build_list: Iter<'a, String>, find_pos: usize) -> Option<(&'a String, usize)> {
    let prefix = "Win_x64/";
    let mut list: Vec<_> = build_list
        .filter_map(|build| {
            if build.starts_with(prefix) {
                if let Ok(build_pos) = (&build[prefix.len()..build.len() - 1]).parse::<usize>() {
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
    chromium_main_branch_position: usize,
    milestone: usize,
    platform: String,
    previous_version: String,
    time: usize,
    version: String,
}
