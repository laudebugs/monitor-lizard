use std::collections::HashSet;
use std::{fs, path::PathBuf};

fn file() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".ownwatch/baseline.json")
}
pub fn load() -> HashSet<String> {
    fs::read_to_string(file()).ok().and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok()).unwrap_or_default().into_iter().collect()
}
pub fn approve(key: &str) -> std::io::Result<()> {
    let mut set = load();
    set.insert(key.to_string());
    let f = file();
    fs::create_dir_all(f.parent().unwrap())?;
    let mut v: Vec<_> = set.into_iter().collect();
    v.sort();
    fs::write(f, serde_json::to_string_pretty(&v).unwrap())
}
