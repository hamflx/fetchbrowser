use std::{fs::File, io::BufReader};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::utils::get_cached_file_path;

pub(crate) struct ChromiumBuilds(Vec<String>);

impl ChromiumBuilds {
    pub(crate) fn init(os: &str) -> Result<Self> {
        let builds_json_path = get_cached_file_path(&format!("builds-{os}.json"))?;
        let build_list = if std::fs::try_exists(&builds_json_path).unwrap_or_default() {
            println!("==> using cached builds.");
            serde_json::from_reader(BufReader::new(File::open(&builds_json_path)?))?
        } else {
            println!("==> retrieving builds ...");
            let pages = ChromiumBuildsPage::new(os)?;
            let mut unwrapped_page_list = Vec::new();
            for page in pages {
                unwrapped_page_list.push(page?);
            }
            let builds: Vec<String> = unwrapped_page_list.into_iter().flatten().collect();
            std::fs::write(&builds_json_path, serde_json::to_string(&builds)?)?;
            builds
        };
        Ok(Self(build_list))
    }

    pub(crate) fn find<'a>(&'a self, find_pos: usize, os_prefix: &str) -> Option<&'a String> {
        let mut list: Vec<_> = self
            .0
            .iter()
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
            .map(|b| b.0)
    }
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
    pub(crate) kind: String,
    pub(crate) next_page_token: Option<String>,
    #[serde(default)]
    pub(crate) prefixes: Vec<String>,
    #[serde(default)]
    pub(crate) items: Vec<GoogleApiStorageObject>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GoogleApiStorageObject {
    pub(crate) kind: String,
    pub(crate) media_link: String,
    pub(crate) name: String,
    pub(crate) size: String,
    pub(crate) updated: String,
}

pub(crate) fn get_os_prefix(os: &str) -> Result<&'static str> {
    match os {
        "windows" => Ok("Win_x64"),
        "macos" => Ok("Mac"),
        _ => Err(anyhow!("不支持的操作系统：{}", os)),
    }
}
