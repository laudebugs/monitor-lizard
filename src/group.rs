use crate::model::*;
use std::collections::{BTreeMap, HashSet};

pub struct Id { pub key: String, pub label: String, pub category: &'static str }

fn base(p: &str) -> &str { p.rsplit('/').next().unwrap_or(p) }

/// Map an executable path to a stable group: app bundle, toolchain, or macOS system.
pub fn group_of(exe: &str) -> Id {
    if let Some(i) = exe.find(".app/") {
        let top = &exe[..i + 4];
        let cat = if exe.starts_with("/System/") { "macOS system" } else if exe.contains("/Applications/") { "Apps" } else { "Apps (other)" };
        return Id { key: top.into(), label: base(top).trim_end_matches(".app").into(), category: cat };
    }
    let b = base(exe);
    let semver = b.split('.').count() == 3 && b.split('.').all(|x| x.parse::<u32>().is_ok());
    if exe.contains("/.local/share/claude") || semver { return Id { key: "dev:claude-code".into(), label: "Claude Code".into(), category: "Developer tools" }; }
    if ["/System/", "/usr/libexec", "/usr/sbin", "/sbin", "/usr/bin", "/bin/"].iter().any(|p| exe.starts_with(p)) {
        return Id { key: format!("apple:{exe}"), label: b.into(), category: "macOS system" };
    }
    if exe.contains("/.nvm/") || exe.contains("/node_modules/") || ["node", "bun", "deno"].contains(&b) {
        return Id { key: "dev:node".into(), label: "Node / JS runtimes".into(), category: "Developer tools" };
    }
    if ["/.cargo/", "/.pyenv/", "/opt/homebrew", "/usr/local"].iter().any(|p| exe.contains(p)) {
        return Id { key: format!("dev:{exe}"), label: b.into(), category: "Developer tools" };
    }
    Id { key: format!("other:{exe}"), label: b.into(), category: "Unclassified" }
}

pub fn build(procs: Vec<Proc>, conns: Vec<Conn>, approved: &HashSet<String>) -> Vec<Group> {
    let exe_by_pid: std::collections::HashMap<u32, String> = procs.iter().map(|p| (p.pid, p.exe.clone())).collect();
    let mut groups: BTreeMap<String, Group> = BTreeMap::new();
    let ensure = |groups: &mut BTreeMap<String, Group>, exe: &str| -> String {
        let id = group_of(exe);
        let key = id.key.clone();
        groups.entry(key.clone()).or_insert_with(|| Group {
            approved: approved.contains(&id.key), key: id.key, label: id.label, category: id.category.into(),
            procs: vec![], conns: vec![], signing: None, trust: Label::Unknown, rules_fired: vec![],
        });
        key
    };
    for p in procs { let k = ensure(&mut groups, &p.exe); groups.get_mut(&k).unwrap().procs.push(p); }
    for c in conns {
        if let Some(exe) = exe_by_pid.get(&c.pid) { let k = ensure(&mut groups, exe); groups.get_mut(&k).unwrap().conns.push(c); }
    }
    let mut v: Vec<Group> = groups.into_values().collect();
    v.sort_by(|a, b| b.conns.len().cmp(&a.conns.len()).then(b.procs.len().cmp(&a.procs.len())));
    v
}
