use std::{cmp::Ordering, env::current_dir, fs::create_dir_all, io::Cursor};

use anyhow::{anyhow, Result};
use bytes::Bytes;
use compress_tools::{uncompress_archive, Ownership};
use select::{
    document::Document,
    predicate::{self, Predicate},
};

use crate::utils::{find_sequence, get_cached_file_path};

pub(crate) fn download_ff(version: &str) -> Result<()> {
    let cur_dir = current_dir()?;

    let spider = FirefoxVersionSpider::init()?;
    let matched_version_list = spider.find(version);
    let matched_version = matched_version_list
        .first()
        .ok_or_else(|| anyhow!("No matched version found"))?;

    let zip_content = download_ff_zip(matched_version, "win64").or_else(|err| {
        println!("==> download firefox win64 failed: {err}, trying win32 ...");
        download_ff_zip(matched_version, "win32")
    })?;

    let base_path = cur_dir.join(format!(".tmp-firefox-{matched_version}"));
    create_dir_all(&base_path)?;

    uncompress_archive(Cursor::new(zip_content), &base_path, Ownership::Preserve)?;

    let ff_path = cur_dir.join(format!("firefox-{matched_version}"));
    if ff_path.exists() {
        std::fs::remove_dir_all(&ff_path)?;
    }
    std::fs::rename(base_path.join("core"), ff_path)?;
    if base_path.exists() {
        std::fs::remove_dir_all(&base_path)?;
    }

    let setup_path = base_path.join("setup.exe");
    if setup_path.exists() {
        std::fs::remove_file(setup_path)?;
    }

    Ok(())
}

pub(crate) fn download_ff_zip(version: &str, arch: &str) -> Result<Bytes> {
    let cur_dir = current_dir()?;
    let url = format!(
        "https://ftp.mozilla.org/pub/firefox/releases/{version}/{arch}/zh-CN/Firefox%20Setup%20{version}.exe"
    );
    println!("==> download firefox: {url}");
    let response = reqwest::blocking::get(url)?;
    if !response.status().is_success() {
        return Err(anyhow!("Download firefox failed: {}", response.status()));
    }
    let exe_response = response.bytes()?;
    let signature = b"7z\xbc\xaf\x27\x1c";
    let index_of_sig = find_sequence(exe_response.as_ref(), signature).ok_or_else(|| {
        let exe_path = cur_dir.join(format!("Firefox Setup {version}.exe"));
        match std::fs::write(&exe_path, exe_response.as_ref()) {
            Ok(_) => anyhow!(
                "No 7zip signature found, setup.exe saved at: {}",
                exe_path.to_str().unwrap_or_default()
            ),
            Err(_) => anyhow!("No 7zip signature found"),
        }
    })?;
    Ok(exe_response.slice(index_of_sig..))
}

#[derive(Debug)]
pub(crate) struct FirefoxVersionSpider(Vec<String>);

impl FirefoxVersionSpider {
    pub(crate) fn init() -> Result<Self> {
        let cached_releases_path = get_cached_file_path("firefox-releases.json")?;
        if cached_releases_path.exists() {
            println!("==> using cached firefox releases");
            let releases = serde_json::from_reader(std::fs::File::open(cached_releases_path)?)?;
            Ok(Self(releases))
        } else {
            println!("==> fetching firefox releases from ftp.mozilla.org ...");
            let response =
                reqwest::blocking::get("https://ftp.mozilla.org/pub/firefox/releases/")?.text()?;
            let doc = Document::from(response.as_str());
            let releases = doc
                .find(
                    predicate::Name("tr")
                        .descendant(predicate::Name("td"))
                        .descendant(predicate::Name("a")),
                )
                .map(|node| node.text().trim_end_matches('/').to_owned())
                .filter(|name| is_valid_version(name.as_str()))
                .collect::<Vec<_>>();

            std::fs::write(&cached_releases_path, serde_json::to_string(&releases)?)?;

            Ok(Self(releases))
        }
    }

    pub(crate) fn find(&self, version: &str) -> Vec<&String> {
        let mut matched_list = self
            .0
            .iter()
            .filter(|v| {
                v.starts_with(version)
                    && match v.chars().nth(version.chars().count()) {
                        None => true,
                        Some(ch) => !ch.is_numeric(),
                    }
            })
            .collect::<Vec<_>>();
        matched_list.sort_by(|a, b| {
            let a_pure_num = a.chars().all(|ch| ch == '.' || ch.is_numeric());
            let b_pure_num = b.chars().all(|ch| ch == '.' || ch.is_numeric());
            match (a_pure_num, b_pure_num) {
                (true, true) | (false, false) => a.cmp(b),
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
            }
        });
        matched_list
    }
}

pub(crate) fn is_valid_version(version: &str) -> bool {
    let mut split = version.split('.');
    match (split.next(), split.next()) {
        (Some(first), Some(second)) => {
            first.parse::<u32>().is_ok() && second.parse::<u32>().is_ok()
        }
        _ => false,
    }
}
