#![feature(fs_try_exists)]

mod chromium;
mod common;
mod firefox;
mod platform;
mod utils;

use std::str::FromStr;

use anyhow::Result;
use chromium::ChromiumReleases;
use clap::Parser;
use common::{BrowserReleaseItem, BrowserReleases};
use firefox::download_firefox;
use platform::{Arch, Os, Platform};
use reqwest::blocking::{Client, ClientBuilder};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    os: Option<String>,

    #[arg()]
    browser_version: String,

    #[arg(long)]
    chrome: bool,

    #[arg(long)]
    firefox: bool,

    #[arg(short, long)]
    proxy: Option<String>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err:?}");
    }
}

fn run() -> Result<()> {
    let args = Args::parse();
    let no_browser_specified = !args.chrome && !args.firefox;
    let proxy = build_proxy_client(args.proxy.as_deref())?;
    if args.chrome || no_browser_specified {
        let os = Os::from_str(args.os.as_deref().unwrap_or(std::env::consts::OS))?;
        let x64platform = Platform::new(os, Arch::X86_64);
        if let Err(err) =
            download_browser::<ChromiumReleases>(x64platform, proxy.clone(), &args.browser_version)
        {
            // todo 这里不要无脑回退下载 x86，应该在版本找不到的时候才下载 x86 版本的。
            let x86platform = Platform::new(os, Arch::X86);
            if !x64platform.eq_impl(&x86platform) {
                println!("==> 下载 x64 版本出错，尝试 x86: {err}");
                download_browser::<ChromiumReleases>(
                    x86platform,
                    proxy.clone(),
                    &args.browser_version,
                )?;
            } else {
                return Err(err);
            }
        }
    }
    if args.firefox {
        download_firefox(&args.browser_version, &proxy)?;
    }
    Ok(())
}

fn build_proxy_client(proxy: Option<&str>) -> Result<Client> {
    let builder = ClientBuilder::new();
    let builder = match proxy {
        Some(proxy) => builder.proxy(reqwest::Proxy::all(proxy)?),
        None => builder,
    };
    Ok(builder.build()?)
}

fn download_browser<B: BrowserReleases>(
    platform: Platform,
    client: Client,
    version: &str,
) -> Result<()> {
    let fetcher = B::init(platform, client)?;
    let matched_version_list = fetcher.match_version(version);
    if let Some(release) = matched_version_list.into_iter().next() {
        release?.download()?;
        return Ok(());
    }
    Err(anyhow::anyhow!("No matched version found."))
}
