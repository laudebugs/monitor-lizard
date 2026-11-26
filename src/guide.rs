//! The one table of commands and terms behind `monitor-lizard help` and `GET /api/guide`.
//! Nothing here runs a command: entries are text a person copies.
use serde::Serialize;

/// What a command does to the Mac, in the words the Guide shows.
#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
pub enum Safety {
    #[serde(rename = "read-only")]
    ReadOnly,
    #[serde(rename = "changes files")]
    ChangesFiles,
    #[serde(rename = "needs sudo")]
    NeedsSudo,
}

impl Safety {
    /// The serialized name, so the terminal and the JSON can never disagree.
    pub fn label(self) -> String {
        serde_json::to_value(self).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
    }
}

/// Real output, trimmed, with identifying details replaced by placeholders.
#[derive(Serialize)]
pub struct Example {
    /// macOS version the output was captured on; formats change between releases.
    pub macos: &'static str,
    pub output: &'static str,
}

#[derive(Serialize)]
pub struct Command {
    pub id: &'static str,
    pub topic: &'static str,
    pub title: &'static str,
    /// Shown as typed. A path or date the user must supply is written in angle brackets.
    pub command: &'static str,
    pub description: &'static str,
    pub example: Option<Example>,
    pub safety: Safety,
    /// Dashboard page that shows the same information.
    pub appears_in: Option<&'static str>,
    pub undo: Option<&'static str>,
    pub tags: &'static [&'static str],
}

#[derive(Serialize)]
pub struct Term {
    pub id: &'static str,
    pub term: &'static str,
    pub meaning: &'static str,
    pub tags: &'static [&'static str],
}

/// Topic of the entries that are `monitor-lizard` subcommands: usage and the completeness test depend on it.
pub const OWN_TOPIC: &str = "monitor-lizard";
const STORAGE: &str = "storage";

const MACOS: &str = "macOS 27.0.1";

pub static COMMANDS: &[Command] = &[
    Command {
        id: "sm-snapshot", topic: OWN_TOPIC, title: "Print the grouped snapshot",
        command: "monitor-lizard snapshot",
        description: "Lists every process and network socket grouped by app, with listening ports and signing state. This is also what plain `monitor-lizard` prints.",
        example: None, safety: Safety::ReadOnly, appears_in: Some("Overview"), undo: None,
        tags: &["processes", "sockets", "groups"],
    },
    Command {
        id: "sm-persistence", topic: OWN_TOPIC, title: "List startup items",
        command: "monitor-lizard persistence",
        description: "Lists launch agents, launch daemons and cron jobs. Items not seen before are marked NEW.",
        example: None, safety: Safety::ReadOnly, appears_in: Some("Persistence"), undo: None,
        tags: &["startup", "launchagent", "launchdaemon", "cron"],
    },
    Command {
        id: "sm-metrics", topic: OWN_TOPIC, title: "Print one metrics sample",
        command: "monitor-lizard metrics",
        description: "Prints one CPU, memory, network, disk and GPU sample as JSON.",
        example: None, safety: Safety::ReadOnly, appears_in: Some("Performance"), undo: None,
        tags: &["cpu", "memory", "disk", "gpu", "json"],
    },
    Command {
        id: "sm-classify", topic: OWN_TOPIC, title: "Explain why a group got its label",
        command: "monitor-lizard classify <group-key>",
        description: "Prints the trust label, the rules that fired and the actions allowed for one app group.",
        example: None, safety: Safety::ReadOnly, appears_in: Some("Apps"), undo: None,
        tags: &["label", "rules", "trust"],
    },
    Command {
        id: "sm-llm-classify", topic: OWN_TOPIC, title: "Ask a local model about a group",
        command: "monitor-lizard llm-classify <group-key> [port=8090]",
        description: "Sends the evidence for one group to a model server on this Mac (default port 8090) and prints its reply. Nothing leaves the machine.",
        example: None, safety: Safety::ReadOnly, appears_in: None, undo: None,
        tags: &["llm", "model", "classify"],
    },
    Command {
        id: "sm-watch", topic: OWN_TOPIC, title: "Watch for sudo and su",
        command: "monitor-lizard watch",
        description: "Prints each sudo or su run, with the chain of parent processes, until you press Ctrl-C. Each event is also saved to the history database.",
        example: None, safety: Safety::ChangesFiles, appears_in: Some("Privilege"),
        undo: Some("Quit and delete ~/.monitor-lizard/history.db to drop the saved events."),
        tags: &["sudo", "su", "privilege"],
    },
    Command {
        id: "sm-sudo-log", topic: OWN_TOPIC, title: "Show recent sudo and su events",
        command: "monitor-lizard sudo-log [n]",
        description: "Prints the last n (default 20) events saved by `watch`.",
        example: None, safety: Safety::ReadOnly, appears_in: Some("Privilege"), undo: None,
        tags: &["sudo", "su", "history"],
    },
    Command {
        id: "sm-privilege", topic: OWN_TOPIC, title: "Show recent privilege prompts",
        command: "monitor-lizard privilege [minutes]",
        description: "Prints authorization attempts from the last n minutes (default 60), including failures and the parent process.",
        example: None, safety: Safety::ReadOnly, appears_in: Some("Privilege"), undo: None,
        tags: &["authd", "authorization", "sudo"],
    },
    Command {
        id: "sm-json", topic: OWN_TOPIC, title: "Print the full snapshot as JSON",
        command: "monitor-lizard json",
        description: "Prints the same data the dashboard loads, for scripts and agents.",
        example: None, safety: Safety::ReadOnly, appears_in: None, undo: None,
        tags: &["json", "export"],
    },
    Command {
        id: "sm-serve", topic: OWN_TOPIC, title: "Start the dashboard",
        command: "monitor-lizard serve [port]",
        description: "Serves the dashboard on 127.0.0.1 (default port 4747). The dashboard can stop your own processes after you confirm, and saves history to ~/.monitor-lizard/.",
        example: None, safety: Safety::ChangesFiles, appears_in: None,
        undo: Some("Press Ctrl-C to stop the server. A stopped process cannot be restarted from the dashboard."),
        tags: &["dashboard", "server", "web"],
    },
    Command {
        id: "sm-approve", topic: OWN_TOPIC, title: "Mark app groups as expected",
        command: "monitor-lizard approve <group-key>...",
        description: "Adds each group to the baseline of expected apps, so it is no longer reported as new.",
        example: None, safety: Safety::ChangesFiles, appears_in: Some("Apps"),
        undo: Some("Remove the key from the baseline file in ~/.monitor-lizard/."),
        tags: &["baseline", "approve", "expected"],
    },
    Command {
        id: "sm-help", topic: OWN_TOPIC, title: "Search this guide",
        command: "monitor-lizard help [word]",
        description: "Prints the commands and terms whose title, command or tags contain the word. With no word it prints all of them.",
        example: None, safety: Safety::ReadOnly, appears_in: Some("Guide"), undo: None,
        tags: &["help", "usage", "guide"],
    },
    Command {
        id: "df-root", topic: STORAGE, title: "Free space on the startup disk",
        command: "df -h /",
        description: "Shows size, used and available space of the startup volume. Used can look small because macOS counts the other volumes in the same container separately.",
        example: Some(Example { macos: MACOS, output: "Filesystem        Size    Used   Avail Capacity iused ifree %iused  Mounted on\n/dev/disk3s1s1   460Gi    13Gi    36Gi    27%    484k  374M    0%   /" }),
        safety: Safety::ReadOnly, appears_in: Some("Storage"), undo: None,
        tags: &["disk", "space", "free", "df"],
    },
    Command {
        id: "diskutil-apfs-list", topic: STORAGE, title: "APFS containers and volumes",
        command: "diskutil apfs list",
        description: "Lists each APFS container with the space its volumes use and how much is not allocated, then every volume with its role and size.",
        example: Some(Example { macos: MACOS, output: "APFS Containers (6 found)\n|\n+-- Container disk3 <uuid>\n|   ====================================================\n|   APFS Container Reference:     disk3\n|   Size (Capacity Ceiling):      494384795648 B (494.4 GB)\n|   Capacity In Use By Volumes:   456152682496 B (456.2 GB) (92.3% used)\n|   Capacity Not Allocated:       38232113152 B (38.2 GB) (7.7% free)" }),
        safety: Safety::ReadOnly, appears_in: Some("Storage"), undo: None,
        tags: &["apfs", "volume", "container", "diskutil"],
    },
    Command {
        id: "tmutil-list", topic: STORAGE, title: "List local Time Machine snapshots",
        command: "tmutil listlocalsnapshots /",
        description: "Lists the dates of local snapshots on the startup volume. It shows dates only, not sizes.",
        example: Some(Example { macos: MACOS, output: "Snapshots for disk /:" }),
        safety: Safety::ReadOnly, appears_in: Some("Storage"), undo: None,
        tags: &["snapshot", "time machine", "tmutil", "local"],
    },
    Command {
        id: "tmutil-delete", topic: STORAGE, title: "Delete one local snapshot",
        command: "tmutil deletelocalsnapshots <date>",
        description: "Deletes every local Time Machine snapshot with that date (format YYYY-MM-DD-HHMMSS, as listed above) on all mounted disks. The man page does not say it needs root; the usual instruction is to run it with sudo and to give the terminal Full Disk Access. Labelled needs sudo as the safe assumption.",
        example: None, safety: Safety::NeedsSudo, appears_in: Some("Storage"),
        undo: Some("None. The snapshot is gone; Time Machine makes a new one at its next run."),
        tags: &["snapshot", "time machine", "tmutil", "delete"],
    },
    Command {
        id: "du-library", topic: STORAGE, title: "Largest folders in your Library",
        command: "du -sh ~/Library/* | sort -h",
        description: "Sums each folder in your Library and sorts them smallest to largest. It can take a minute on a large Library. The example shortens the absolute paths that `du` prints to `~`.",
        example: Some(Example { macos: MACOS, output: " 11G\t~/Library/Containers\n 19G\t~/Library/pnpm\n 29G\t~/Library/Caches\n 33G\t~/Library/Application Support" }),
        safety: Safety::ReadOnly, appears_in: Some("Storage"), undo: None,
        tags: &["library", "folders", "size", "du", "caches"],
    },
    Command {
        id: "simctl-delete-unavailable", topic: STORAGE, title: "Delete unusable Xcode simulators",
        command: "xcrun simctl delete unavailable",
        description: "Deletes simulator devices that the installed Xcode no longer supports, and their data.",
        example: None, safety: Safety::ChangesFiles, appears_in: Some("Storage"),
        undo: Some("None. Create new simulators in Xcode if you need them again."),
        tags: &["xcode", "simulator", "simctl", "developer"],
    },
    Command {
        id: "docker-df", topic: STORAGE, title: "Docker disk use",
        command: "docker system df",
        description: "Shows the space used by images, containers, local volumes and build cache. It needs the Docker daemon to be running.",
        example: None, safety: Safety::ReadOnly, appears_in: Some("Storage"), undo: None,
        tags: &["docker", "images", "containers", "developer"],
    },
    Command {
        id: "docker-prune", topic: STORAGE, title: "Remove unused Docker data",
        command: "docker system prune",
        description: "Asks for confirmation, then removes stopped containers, networks no container uses, dangling images and dangling build cache. `--all` also removes every unused image, `--volumes` also removes anonymous volumes, and `-f` skips the question.",
        example: None, safety: Safety::ChangesFiles, appears_in: Some("Storage"),
        undo: Some("None. Images can be pulled or rebuilt again; files in removed containers and anonymous volumes are lost."),
        tags: &["docker", "prune", "cleanup", "developer"],
    },
    Command {
        id: "brew-cleanup-n", topic: STORAGE, title: "Preview Homebrew clean-up",
        command: "brew cleanup -n",
        description: "Lists the old versions and cached downloads that `brew cleanup` would remove, and how much space that frees. The -n flag removes nothing.",
        example: Some(Example { macos: MACOS, output: "Would remove: /opt/homebrew/Library/Homebrew/vendor/portable-ruby/<version> (1,707 files, 34.6MB)\n==> This operation would free approximately 69.3MB of disk space." }),
        safety: Safety::ReadOnly, appears_in: Some("Storage"), undo: None,
        tags: &["homebrew", "brew", "cleanup", "developer"],
    },
];

pub static TERMS: &[Term] = &[
    Term { id: "apfs", term: "APFS", meaning: "Apple File System, the format of the startup disk. Volumes in one container share its free space, so each volume's size changes as the others grow.", tags: &["disk", "volume"] },
    Term { id: "local-snapshot", term: "Local snapshot", meaning: "A copy of the disk's state that Time Machine keeps on the Mac. macOS removes it when space runs low.", tags: &["snapshot", "time machine"] },
    Term { id: "purgeable", term: "Purgeable space", meaning: "Space held by caches and snapshots that macOS frees on demand. It counts as available in Finder and as used in `df`.", tags: &["space", "cache"] },
    Term { id: "system-data", term: "System Data", meaning: "The Storage settings category for everything that is not an app, document or media file: caches, logs, snapshots and developer files.", tags: &["storage", "settings"] },
    Term { id: "full-disk-access", term: "Full Disk Access", meaning: "A privacy permission an app needs before it can read protected folders such as Mail and Messages.", tags: &["permission", "privacy"] },
    Term { id: "launch-agent", term: "Launch agent", meaning: "A property-list file that starts a program when you log in. Launch daemons do the same at boot, with system rights.", tags: &["startup", "launchagent", "launchdaemon"] },
    Term { id: "listener", term: "Listener", meaning: "A process waiting for incoming network connections on a port. A star marks one reachable from other machines.", tags: &["port", "network", "socket"] },
    Term { id: "group", term: "App group", meaning: "All processes and sockets that belong to one app, shown as one row.", tags: &["app", "process"] },
    Term { id: "trust-label", term: "Trust label", meaning: "The verdict a rule gives a group, such as yours, Apple or unknown. Rules set it, never a model.", tags: &["label", "rules", "classify"] },
];

/// Every subcommand, in table order, as `usage:` shows them.
pub fn usage() -> String {
    let parts: Vec<String> = COMMANDS.iter().filter(|c| c.topic == OWN_TOPIC).map(|c| c.command.trim_start_matches("monitor-lizard ").to_string()).collect();
    format!("usage: monitor-lizard [{}]", parts.join(" | "))
}

fn contains(hay: &str, word: &str) -> bool {
    hay.to_lowercase().contains(word)
}

/// Entries whose title, command or tags contain `word`, ignoring case. A blank word matches everything.
pub fn search(word: &str) -> (Vec<&'static Command>, Vec<&'static Term>) {
    let w = word.trim().to_lowercase();
    let cmds = COMMANDS.iter().filter(|c| contains(c.title, &w) || contains(c.command, &w) || c.tags.iter().any(|t| contains(t, &w))).collect();
    let terms = TERMS.iter().filter(|t| contains(t.term, &w) || t.tags.iter().any(|g| contains(g, &w))).collect();
    (cmds, terms)
}

fn topics() -> Vec<&'static str> {
    let mut t: Vec<&str> = Vec::new();
    for c in COMMANDS { if !t.contains(&c.topic) { t.push(c.topic); } }
    t
}

/// The text `monitor-lizard help [word]` prints.
pub fn render_help(word: &str) -> String {
    let (cmds, terms) = search(word);
    if cmds.is_empty() && terms.is_empty() {
        return format!("No command matches \"{}\". Topics: {}", word.trim(), topics().join(", "));
    }
    let mut out = String::new();
    for c in cmds {
        out += &format!("{}  [{}]\n  {}\n  {}\n", c.title, c.safety.label(), c.command, c.description);
        if let Some(u) = c.undo { out += &format!("  Undo: {u}\n"); }
        if let Some(e) = &c.example { out += &format!("  Example ({}):\n{}\n", e.macos, e.output.lines().map(|l| format!("    {l}")).collect::<Vec<_>>().join("\n")); }
        out.push('\n');
    }
    for t in terms { out += &format!("{}\n  {}\n\n", t.term, t.meaning); }
    out.trim_end().to_string()
}

/// The body of `GET /api/guide`.
pub fn to_json() -> String {
    serde_json::json!({ "commands": COMMANDS, "terms": TERMS }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Subcommand names from the dispatch `match` in `main.rs`. Only plain `Some("name") =>` arms
    /// (optionally `None | Some("name")`) are understood; any other arm shape is an error, so a
    /// subcommand written another way fails loudly instead of slipping past the check.
    fn parse_arms(src: &str) -> Result<Vec<String>, String> {
        let start = src.find("match args.next().as_deref() {").ok_or("dispatch match not found in main.rs")?;
        let mut names = Vec::new();
        for line in src[start..].lines().skip(1) {
            if line.starts_with("    }") { break; }
            let Some(arm) = line.strip_prefix("        ").filter(|a| !a.starts_with(' ')) else { continue };
            if arm.starts_with("//") || arm.starts_with('}') { continue; }
            let Some((pat, _)) = arm.split_once(" =>") else { return Err(format!("cannot read dispatch arm: {arm}")) };
            let pat = pat.strip_prefix("None | ").unwrap_or(pat);
            if pat == "_" { continue; }
            match pat.strip_prefix("Some(\"").and_then(|p| p.strip_suffix("\")")).filter(|n| !n.contains('"')) {
                Some(n) => names.push(n.to_string()),
                None => return Err(format!("unsupported dispatch arm `{pat}`: write it as Some(\"name\") or extend parse_arms")),
            }
        }
        Ok(names)
    }

    fn main_subcommands() -> Vec<String> {
        parse_arms(include_str!("main.rs")).unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn unsupported_arm_shapes_are_rejected() {
        let wrap = |arm: &str| format!("    match args.next().as_deref() {{\n        {arm} => run(),\n    }}\n");
        assert_eq!(parse_arms(&wrap("Some(\"a\")")).unwrap(), ["a"]);
        assert_eq!(parse_arms(&wrap("None | Some(\"b\")")).unwrap(), ["b"]);
        for bad in ["Some(\"a\" | \"b\")", "Some(x) if x == \"c\"", "\"d\""] {
            assert!(parse_arms(&wrap(bad)).is_err(), "{bad} should be rejected");
        }
    }

    fn own_names() -> Vec<&'static str> {
        COMMANDS.iter().filter(|c| c.topic == OWN_TOPIC).map(|c| c.command.split_whitespace().nth(1).unwrap()).collect()
    }

    #[test]
    fn every_subcommand_has_an_entry() {
        let names = own_names();
        let subs = main_subcommands();
        assert!(subs.len() >= 10, "parsed too few subcommands: {subs:?}");
        for s in &subs { assert!(names.contains(&s.as_str()), "main.rs handles `{s}` but the guide table has no entry: add an entry to COMMANDS in src/guide.rs"); }
    }

    #[test]
    fn every_own_entry_is_a_real_subcommand() {
        let subs = main_subcommands();
        for n in own_names() { assert!(subs.iter().any(|s| s == n), "guide lists `{n}` but main.rs does not handle it: remove the entry from COMMANDS in src/guide.rs or add the arm"); }
    }

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<_> = COMMANDS.iter().map(|c| c.id).chain(TERMS.iter().map(|t| t.id)).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }

    #[test]
    fn search_matches_title_command_and_tags() {
        let (c, _) = search("snapshot");
        let ids: Vec<_> = c.iter().map(|c| c.id).collect();
        assert!(ids.contains(&"tmutil-list") && ids.contains(&"tmutil-delete") && ids.contains(&"sm-snapshot"));
        assert!(search("SIMCTL").0.iter().any(|c| c.id == "simctl-delete-unavailable"), "search ignores case and reads the command");
        assert!(search("prune").0.iter().any(|c| c.id == "docker-prune"), "search reads tags");
    }

    #[test]
    fn no_match_names_the_topics() {
        let text = render_help("zzzz");
        assert!(text.starts_with("No command matches \"zzzz\""));
        assert!(text.contains("monitor-lizard") && text.contains("storage"));
    }

    #[test]
    fn usage_lists_every_subcommand() {
        let u = usage();
        for n in own_names() { assert!(u.contains(n), "{u} misses {n}"); }
    }

    #[test]
    fn commands_that_take_a_path_or_date_use_placeholders() {
        let c = COMMANDS.iter().find(|c| c.id == "tmutil-delete").unwrap();
        assert!(c.command.contains("<date>"));
        assert_eq!(c.safety, Safety::NeedsSudo);
    }

    #[test]
    fn text_has_no_identifying_details() {
        let json = to_json();
        assert!(!json.contains("/Users/"), "guide text contains a home path");
        if let Some(user) = std::env::var("USER").ok().filter(|u| u.len() > 2) { assert!(!json.contains(&format!("/{user}")), "guide text contains the account name"); }
    }

    #[test]
    fn json_has_both_tables_with_safety_text() {
        let v: serde_json::Value = serde_json::from_str(&to_json()).unwrap();
        assert_eq!(v["commands"].as_array().unwrap().len(), COMMANDS.len());
        assert_eq!(v["terms"].as_array().unwrap().len(), TERMS.len());
        assert!(v["commands"].as_array().unwrap().iter().any(|c| c["safety"] == "needs sudo"));
    }
}
