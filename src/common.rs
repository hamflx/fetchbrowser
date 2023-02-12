use anyhow::Result;

use crate::platform::Platform;

pub(crate) trait BrowserReleases {
    type ReleaseItem: BrowserReleaseItem;
    type Matches<'r>: Iterator<Item = Result<Self::ReleaseItem>>
    where
        Self: 'r;

    fn init(platform: Platform) -> Result<Self>
    where
        Self: Sized;

    fn match_version<'r>(&'r self, version: &str) -> Self::Matches<'r>;
}

pub(crate) trait BrowserReleaseItem {
    fn download(&self) -> Result<()>;
}
