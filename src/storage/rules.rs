//! The rules table: one label per path. A wrong `safe` tells a user to delete something, so the
//! `safe` allowlist is checked after every rule that could rule it out, and anything unclear
//! (relative, `..`, not UTF-8, no home folder, a symlink out of the tree) is `review`.
use serde::Serialize;
use std::path::{Component, Path};

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Label {
    /// An app rebuilds or re-downloads it.
    Safe,
    /// Removing it has side effects the user should weigh.
    Review,
    /// The user's own files.
    Yours,
}

/// The rule that gave a label; the page shows it as the reason.
#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Rule {
    UserContent,
    Mail,
    XcodeDerivedData,
    HomebrewCache,
    PackageManagerCache,
    AppCache,
    DockerData,
    Simulators,
    Logs,
    IdleNodeModules,
    NodeModules,
    Applications,
    OutsideHome,
    UnclearPath,
    NoHome,
    Symlink,
    Unclassified,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
pub struct Verdict { pub label: Label, pub rule: Rule }

fn verdict(label: Label, rule: Rule) -> Verdict { Verdict { label, rule } }

/// Folders that hold the user's own files, lowercase: nothing under them can be `safe`.
pub const USER_CONTENT: &[&str] = &["documents", "desktop", "pictures", "movies", "music", "downloads"];

/// Caches of package managers that download again on demand. Each entry has a reason; an app's own
/// cache is not on this list because some apps keep offline data there.
const REBUILDABLE_CACHES: &[(&str, Rule)] = &[
    // Homebrew re-downloads bottles on the next install.
    ("homebrew", Rule::HomebrewCache),
    // pip re-downloads wheels it needs.
    ("pip", Rule::PackageManagerCache),
    // Yarn re-downloads packages it needs.
    ("yarn", Rule::PackageManagerCache),
    // node-gyp re-downloads Node headers when it builds an add-on.
    ("node-gyp", Rule::PackageManagerCache),
];

/// Path components as lowercase text (APFS is case-insensitive by default), or `None` when the path
/// is relative, climbs with `..` or `.`, or has a component that is not UTF-8.
fn lowercase_parts(path: &Path) -> Option<Vec<String>> {
    if !path.is_absolute() { return None; }
    let mut out = Vec::new();
    for c in path.components() {
        match c {
            Component::RootDir => {}
            Component::Normal(n) => out.push(n.to_str()?.to_lowercase()),
            // `a/../b` would match a prefix rule for `a` and then point somewhere else.
            _ => return None,
        }
    }
    Some(out)
}

/// The label for one path. Every path gets exactly one; the order is the safety argument: the guards
/// and the user's own files come first, then every rule that says `review`, and `safe` rules last.
/// `project_idle` says whether the project that owns a `node_modules` folder is idle (see `walk`).
pub fn label_for(home: &Path, path: &Path, project_idle: bool) -> Verdict {
    let (Some(h), Some(p)) = (lowercase_parts(home), lowercase_parts(path)) else { return verdict(Label::Review, Rule::UnclearPath) };
    if h.is_empty() { return verdict(Label::Review, Rule::NoHome); }
    let Some(rel) = p.strip_prefix(h.as_slice()) else {
        return if p.first().is_some_and(|f| f == "applications") { verdict(Label::Review, Rule::Applications) } else { verdict(Label::Review, Rule::OutsideHome) };
    };
    let under = |prefix: &[&str]| rel.len() >= prefix.len() && rel.iter().zip(prefix).all(|(a, b)| a == b);
    if rel.first().is_some_and(|f| USER_CONTENT.contains(&f.as_str())) { return verdict(Label::Yours, Rule::UserContent); }
    if under(&["library", "mail"]) { return verdict(Label::Yours, Rule::Mail); }
    // Everything that needs review comes before any `safe` rule, so a `safe` pattern can never
    // reach into one of these (for example `Logs/x/node_modules`).
    if under(&["library", "containers", "com.docker.docker"]) { return verdict(Label::Review, Rule::DockerData); }
    if under(&["library", "developer", "coresimulator"]) { return verdict(Label::Review, Rule::Simulators); }
    if under(&["library", "logs"]) { return verdict(Label::Review, Rule::Logs); }
    if under(&["library", "developer", "xcode", "deriveddata"]) { return verdict(Label::Safe, Rule::XcodeDerivedData); }
    if rel.len() >= 3 && under(&["library", "caches"]) {
        return match REBUILDABLE_CACHES.iter().find(|(id, _)| rel[2] == *id) {
            Some((_, rule)) => verdict(Label::Safe, *rule),
            None => verdict(Label::Review, Rule::AppCache),
        };
    }
    if rel.iter().any(|c| c == "node_modules") {
        return if project_idle { verdict(Label::Safe, Rule::IdleNodeModules) } else { verdict(Label::Review, Rule::NodeModules) };
    }
    verdict(Label::Review, Rule::Unclassified)
}

/// `label_for`, then a check against the real file system: a `safe` label stands only if the path,
/// with every symlink resolved, would get the same label. A symlink into the user's files, or one
/// that points anywhere else, is `review`.
pub fn label_item(home: &Path, path: &Path, project_idle: bool) -> Verdict {
    let v = label_for(home, path, project_idle);
    if v.label != Label::Safe { return v; }
    let resolved = std::fs::canonicalize(path).and_then(|p| std::fs::canonicalize(home).map(|h| (h, p)));
    match resolved {
        Ok((h, p)) if label_for(&h, &p, project_idle) == v => v,
        _ => verdict(Label::Review, Rule::Symlink),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::path::PathBuf;

    const HOME: &str = "/h";

    fn label(rel: &str, idle: bool) -> Verdict { label_for(Path::new(HOME), &Path::new(HOME).join(rel), idle) }

    #[test]
    fn allowlisted_locations_are_safe() {
        for (rel, rule) in [
            ("Library/Developer/Xcode/DerivedData", Rule::XcodeDerivedData),
            ("Library/Developer/Xcode/DerivedData/App-abc/Build", Rule::XcodeDerivedData),
            ("Library/Caches/Homebrew", Rule::HomebrewCache),
            ("Library/Caches/pip/wheels", Rule::PackageManagerCache),
            ("Library/Caches/Yarn", Rule::PackageManagerCache),
        ] {
            assert_eq!(label(rel, false), verdict(Label::Safe, rule), "{rel}");
        }
    }

    #[test]
    fn a_generic_app_cache_needs_review() {
        for rel in ["Library/Caches/com.example.app", "Library/Caches/com.spotify.client/Data"] {
            assert_eq!(label(rel, true), verdict(Label::Review, Rule::AppCache), "{rel}");
        }
    }

    /// The invariant, stated over every user-content folder: spelled in any case, reached with
    /// `..`, or with a safe-looking pattern inside it, the label is never `safe`.
    #[test]
    fn nothing_under_user_content_is_ever_safe() {
        let tails = ["", "/x", "/Library/Caches/Homebrew", "/Library/Caches/pip", "/proj/node_modules", "/proj/node_modules/pkg", "/Library/Developer/Xcode/DerivedData/a"];
        for dir in USER_CONTENT {
            let variants = [dir.to_string(), dir.to_uppercase(), format!("{}{}", &dir[..1].to_uppercase(), &dir[1..]), dir.chars().enumerate().map(|(i, c)| if i % 2 == 0 { c.to_ascii_uppercase() } else { c }).collect()];
            for v in &variants {
                for tail in tails {
                    for idle in [false, true] {
                        for path in [format!("{HOME}/{v}{tail}"), format!("{HOME}/{v}{tail}/"), format!("{HOME}/Library/Caches/../{v}{tail}"), format!("{HOME}/./{v}{tail}")] {
                            let l = label_for(Path::new(HOME), Path::new(&path), idle).label;
                            assert_ne!(l, Label::Safe, "{path}");
                            if !path.contains("..") && !path.contains("/./") { assert_eq!(l, Label::Yours, "{path}"); }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_review_rule_wins_over_node_modules_inside_it() {
        for rel in ["Library/Logs/x/node_modules", "Library/Containers/com.docker.docker/Data/node_modules", "Library/Developer/CoreSimulator/Devices/a/node_modules"] {
            assert_eq!(label(rel, true).label, Label::Review, "{rel}");
        }
    }

    #[test]
    fn a_folder_that_only_starts_with_a_user_folder_name_is_not_user_content() {
        assert_eq!(label("Documents2/node_modules", true).label, Label::Safe);
        assert_eq!(label("Downloads-old", false).label, Label::Review);
    }

    #[test]
    fn mail_is_yours_and_unknown_paths_need_review() {
        assert_eq!(label("Library/Mail", false), verdict(Label::Yours, Rule::Mail));
        assert_eq!(label("library/MAIL/V10/x", true).label, Label::Yours);
        for rel in ["Library/Logs", "Library/Containers/com.docker.docker/Data", "Library/Developer/CoreSimulator/Devices", "Library/Caches", "Something/Else"] {
            assert_eq!(label(rel, false).label, Label::Review, "{rel}");
        }
        assert_eq!(label_for(Path::new(HOME), Path::new("/Applications"), false).rule, Rule::Applications);
        assert_eq!(label_for(Path::new(HOME), Path::new("/var/db/x"), false).rule, Rule::OutsideHome);
    }

    #[test]
    fn node_modules_is_safe_only_when_the_project_is_idle() {
        assert_eq!(label("code/app/node_modules", true), verdict(Label::Safe, Rule::IdleNodeModules));
        assert_eq!(label("code/app/node_modules", false), verdict(Label::Review, Rule::NodeModules));
    }

    #[test]
    fn unclear_paths_and_a_missing_home_are_never_safe() {
        for p in ["/h/Library/Caches/../Documents/x", "/h/Library/Developer/Xcode/DerivedData/../../../../Desktop", "relative/Library/Caches/pip"] {
            assert_eq!(label_for(Path::new(HOME), Path::new(p), true), verdict(Label::Review, Rule::UnclearPath), "{p}");
        }
        let odd: PathBuf = [OsStr::new("/h/Library/Caches/pip/"), OsStr::from_bytes(b"\xff\xfe")].iter().collect();
        assert_eq!(label_for(Path::new(HOME), &odd, true).label, Label::Review, "a component that is not UTF-8");
        for home in ["", "/", "relative"] {
            assert_ne!(label_for(Path::new(home), Path::new("/Library/Caches/pip"), true).label, Label::Safe, "home {home:?}");
        }
    }

    #[test]
    fn a_symlink_into_user_content_is_never_safe() {
        let d = std::env::temp_dir().join(format!("ml-rules-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let home = d.join("home");
        std::fs::create_dir_all(home.join("Documents/stuff")).unwrap();
        std::fs::create_dir_all(home.join("Library/Caches")).unwrap();
        std::fs::create_dir_all(home.join("Library/Developer/Xcode")).unwrap();
        std::fs::create_dir_all(home.join("elsewhere")).unwrap();
        std::os::unix::fs::symlink(home.join("Documents/stuff"), home.join("Library/Caches/pip")).unwrap();
        std::os::unix::fs::symlink(home.join("elsewhere"), home.join("Library/Developer/Xcode/DerivedData")).unwrap();
        std::fs::create_dir_all(home.join("Library/Caches/Yarn")).unwrap();
        assert_eq!(label_item(&home, &home.join("Library/Caches/pip"), false), verdict(Label::Review, Rule::Symlink));
        assert_eq!(label_item(&home, &home.join("Library/Developer/Xcode/DerivedData"), false), verdict(Label::Review, Rule::Symlink));
        // A symlinked parent is caught too.
        assert_eq!(label_item(&home, &home.join("Library/Caches/pip/inside"), false).label, Label::Review);
        // A real folder keeps its label, even though the temp folder itself sits behind a symlink.
        assert_eq!(label_item(&home, &home.join("Library/Caches/Yarn"), false).label, Label::Safe);
        std::fs::remove_dir_all(&d).unwrap();
    }
}
