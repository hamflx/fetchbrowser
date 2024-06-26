use std::vec::IntoIter;

use anyhow::{anyhow, Result};
use reqwest::blocking::Client;

use crate::{
    common::{BrowserReleaseItem, BrowserReleases, ReleaseChannel},
    platform::Platform,
};

use self::{
    builds::{fetch_build_detail, ChromiumBuilds},
    download::download_chromium_zip_file,
    history::{ChromiumHistory, ChromiumHistoryInfo},
};

mod builds;
mod download;
mod history;
mod version;

pub(crate) struct ChromiumReleases {
    platform: Platform,
    history: ChromiumHistory,
    builds: ChromiumBuilds,
    client: Client,
}

impl BrowserReleases for ChromiumReleases {
    type ReleaseItem = ChromiumReleaseItem;
    type Matches<'r> = ChromiumReleaseMatches<'r>;

    fn init(platform: Platform, channel: ReleaseChannel, client: Client) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        // history.json 包含了 base_position 和版本号。
        let history = ChromiumHistory::init(platform, channel, client.clone())?;
        // builds 包含了所有可下载的 position 信息。
        let builds = ChromiumBuilds::init(platform, client.clone())?;
        Ok(Self {
            platform,
            history,
            builds,
            client,
        })
    }

    fn match_version<'r>(&'r self, version: &str) -> Self::Matches<'r> {
        ChromiumReleaseMatches::new(self, self.history.find(version))
    }
}

pub(crate) struct ChromiumReleaseMatches<'r> {
    iter: IntoIter<&'r ChromiumHistoryInfo>,
    releases: &'r ChromiumReleases,
    prefix: &'static str,
}

impl<'r> ChromiumReleaseMatches<'r> {
    fn new(releases: &'r ChromiumReleases, items: Vec<&'r ChromiumHistoryInfo>) -> Self {
        let prefix = releases.platform.prefix();
        Self {
            releases,
            iter: items.into_iter(),
            prefix,
        }
    }
}

impl<'r> Iterator for ChromiumReleaseMatches<'r> {
    type Item = Result<ChromiumReleaseItem>;

    fn next(&mut self) -> Option<Self::Item> {
        for history in self.iter.by_ref() {
            match history.chromium_main_branch_position {
                Some(pos) => match self.releases.builds.find(pos, self.prefix) {
                    Some(rev_prefix) => {
                        return Some(Ok(ChromiumReleaseItem {
                            rev_prefix: rev_prefix.clone(),
                            version: history.version.clone(),
                            client: self.releases.client.clone(),
                        }))
                    }
                    None => println!("==> no build found for rev: {pos}"),
                },
                None => println!(
                    "==> chromium {}: no chromium_base_position.",
                    history.version
                ),
            }
        }
        None
    }
}

pub(crate) struct ChromiumReleaseItem {
    rev_prefix: String,
    version: String,
    client: Client,
}

impl BrowserReleaseItem for ChromiumReleaseItem {
    fn download(&self) -> Result<()> {
        // 根据 prefix 找到该版本文件列表，以及 chrome-win.zip 文件信息。
        let build_files = fetch_build_detail(&self.rev_prefix, &self.client)?;
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
                self.rev_prefix
            )
        })?;

        // 先保存到临时目录里面，待解压的时候，找到里面的版本信息，再重命名一下文件夹。
        let base_path = std::env::current_dir()?.join(format!("chromium-{}", self.version));
        std::fs::create_dir_all(&base_path)?;
        download_chromium_zip_file(zip_file, &base_path, &self.client)
    }
}
