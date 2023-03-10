use std::{fs::File, io::BufReader};

use anyhow::Result;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::{common::ReleaseChannel, platform::Platform, utils::get_cached_file_path};

pub(crate) struct ChromiumHistory(Vec<ChromiumHistoryInfo>);

impl ChromiumHistory {
    pub(crate) fn init(
        platform: Platform,
        channel: ReleaseChannel,
        client: Client,
    ) -> Result<Self> {
        let os_arg = platform.arg_name();
        let channel = channel.as_constant();
        let history_json_path = get_cached_file_path(&format!("history-{os_arg}-{channel}.json"))?;
        let history_list = if std::fs::try_exists(&history_json_path).unwrap_or_default() {
            println!("==> using cached history: {}", history_json_path.display());
            serde_json::from_reader(BufReader::new(File::open(&history_json_path)?))?
        } else {
            println!("==> retrieving history.json ...");
            let url = format!(
                "https://omahaproxy.appspot.com/history.json?os={os_arg}&channel={channel}"
            );
            let response = client.get(url).send()?;
            let history_list: Vec<ChromiumHistoryInfo> = serde_json::from_reader(response)?;
            std::fs::write(&history_json_path, serde_json::to_string(&history_list)?)?;
            history_list
        };
        Ok(Self(history_list))
    }

    pub(crate) fn find<'a>(&'a self, version: &str) -> Vec<&'a ChromiumHistoryInfo> {
        let ver_len = version.len();
        self.0
            .iter()
            .filter(|info| {
                info.version == version
                    || (info.version.chars().nth(ver_len) == Some('.')
                        && info.version.starts_with(version))
            })
            .collect()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ChromiumHistoryInfo {
    pub(crate) channel: String,
    pub(crate) os: String,
    pub(crate) timestamp: String,
    pub(crate) version: String,
}

impl ChromiumHistoryInfo {
    pub(crate) fn deps(&self, client: &Client) -> Result<ChromiumDepsInfo> {
        let url = format!(
            "https://omahaproxy.appspot.com/deps.json?version={}",
            self.version
        );
        println!("==> fetching deps {url} ...");
        let response = client.get(url).send()?;
        Ok(serde_json::from_reader(response)?)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ChromiumDepsInfo {
    pub(crate) chromium_base_commit: Option<String>,
    pub(crate) chromium_base_position: Option<String>,
    pub(crate) chromium_branch: Option<String>,
    pub(crate) chromium_commit: String,
    pub(crate) chromium_version: String,
    pub(crate) skia_commit: String,
    pub(crate) v8_commit: String,
    pub(crate) v8_position: String,
    pub(crate) v8_version: String,
}
