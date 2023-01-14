use std::str::FromStr;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ChromiumVersion(usize, usize, usize, usize);

impl FromStr for ChromiumVersion {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split: Vec<_> = s.split('.').collect();
        if split.len() != 4 {
            Err("无效的版本长度。")
        } else if let &[major, minor, branch, patch] = split
            .into_iter()
            .filter_map(|v| v.parse::<usize>().ok())
            .collect::<Vec<_>>()
            .as_slice()
        {
            Ok(Self(major, minor, branch, patch))
        } else {
            Err("无效的版本长度。")
        }
    }
}

impl ToString for ChromiumVersion {
    fn to_string(&self) -> String {
        format!("{}.{}.{}.{}", self.0, self.1, self.2, self.3)
    }
}
