use std::str::FromStr;

use anyhow::anyhow;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub(crate) struct Platform(Os, Arch);

impl Platform {
    pub(crate) fn new(os: Os, arch: Arch) -> Self {
        Self(os, arch)
    }

    pub(crate) fn prefix(&self) -> &'static str {
        match (self.0, self.1) {
            (Os::Windows, Arch::X86) => "Win",
            (Os::Windows, Arch::X86_64) => "Win_x64",
            (Os::Linux, Arch::X86) => "Linux",
            (Os::Linux, Arch::X86_64) => "Linux_x64",
            (Os::Mac, Arch::X86) => "Mac",
            (Os::Mac, Arch::X86_64) => "Mac",
        }
    }

    pub(crate) fn arg_name(&self) -> &'static str {
        match (self.0, self.1) {
            (Os::Windows, Arch::X86) => "win",
            (Os::Windows, Arch::X86_64) => "win64",
            (Os::Linux, Arch::X86) => "linux",
            (Os::Linux, Arch::X86_64) => "linux",
            (Os::Mac, Arch::X86) => "mac",
            (Os::Mac, Arch::X86_64) => "mac",
        }
    }

    pub(crate) fn eq_impl(&self, other: &Self) -> bool {
        self.prefix() == other.prefix() && self.arg_name() == other.arg_name()
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub(crate) enum Os {
    Windows,
    Linux,
    Mac,
}

impl FromStr for Os {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "windows" => Ok(Self::Windows),
            "linux" => Ok(Self::Linux),
            "macos" => Ok(Self::Mac),
            _ => Err(anyhow!("Unsupported OS: {}", s)),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub(crate) enum Arch {
    X86,
    X86_64,
}
