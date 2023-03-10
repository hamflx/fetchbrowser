use anyhow::Result;
use clap::ValueEnum;
use reqwest::blocking::Client;

use crate::platform::Platform;

pub(crate) trait BrowserReleases {
    type ReleaseItem: BrowserReleaseItem;
    type Matches<'r>: Iterator<Item = Result<Self::ReleaseItem>>
    where
        Self: 'r;

    fn init(platform: Platform, channel: ReleaseChannel, client: Client) -> Result<Self>
    where
        Self: Sized;

    fn match_version<'r>(&'r self, version: &str) -> Self::Matches<'r>;
}

pub(crate) trait BrowserReleaseItem {
    fn download(&self) -> Result<()>;
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, ValueEnum)]
pub(crate) enum ReleaseChannel {
    Stable,
    Beta,
    Dev,
    Canary,
}

impl ReleaseChannel {
    pub(crate) fn as_constant(&self) -> &'static str {
        match self {
            ReleaseChannel::Stable => "stable",
            ReleaseChannel::Beta => "beta",
            ReleaseChannel::Dev => "dev",
            ReleaseChannel::Canary => "canary",
        }
    }
}
