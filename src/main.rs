use monitor_lizard::{baseline, classify, guide, history, llm, model, model_store, persistence, privilege, sampler, server, settings, snapshot, storage, watcher};
use std::collections::BTreeMap;

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("serve") => server::serve(args.next().and_then(|p| p.parse().ok()).unwrap_or(4747)),
        Some("privilege") => {
            let live = history::open_or_report().and_then(|h| h.recent_sudo(500).ok()).unwrap_or_default();
            for e in privilege::recent(args.next().and_then(|m| m.parse().ok()).unwrap_or(60), &live) {
                let who = match e.source {
                    privilege::Source::Captured => e.chain.iter().map(|h| watcher::hop_label(&h.command, &h.exe)).collect::<Vec<_>>().join(" < "),
                    privilege::Source::Ps => watcher::visible(&e.parent.unwrap_or_default()),
                    privilege::Source::None => privilege::UNKNOWN_REQUESTER.to_string(),
                };
                println!("{} {} {} right={} {} parent={}", e.time, if e.ok { "OK  " } else { "FAIL" }, e.client, e.right, e.error.unwrap_or_default(), who);
            }
        }
        Some("watch") => {
            let h = history::open_or_report();
            println!("watching for sudo/su (Ctrl-C to stop)...");
            watcher::watch(|e| {
                println!("[{}] {} pid={} {}", e.ts, e.tool, e.pid, watcher::visible(&e.args));
                for (i, hop) in e.chain.iter().enumerate() { println!("  {}{} {} {}", "  ".repeat(i), if i == 0 { "parent" } else { "└" }, hop.pid, watcher::visible(if hop.command.is_empty() { &hop.exe } else { &hop.command })); if let Some(d) = &hop.cwd { println!("  {}  in {}", "  ".repeat(i), watcher::visible(d)); } if hop.more { println!("  {}  (chain cut at {} hops)", "  ".repeat(i), watcher::MAX_HOPS); } }
                if let Some(h) = &h { let _ = h.record_sudo(&e); }
            })
        }
        Some("sudo-log") => if let Some(h) = history::open_or_report() {
            for e in h.recent_sudo(args.next().and_then(|n| n.parse().ok()).unwrap_or(20)).unwrap_or_default() {
                println!("[{}] {} pid={} {}\n   via {}", e.ts, e.tool, e.pid, watcher::visible(&e.args), e.chain.iter().map(|h| watcher::hop_label(&h.command, &h.exe)).collect::<Vec<_>>().join(" <- "));
            }
        },
        Some("persistence") => for i in persistence::take().items {
            let state = if i.removed_at.is_some() { "GONE" } else if i.is_new { "NEW " } else { "    " };
            println!("{state}{:<12} {:<7} {} -> {}", i.kind, i.scope, i.label, i.program);
        },
        Some("storage") => println!("{}", storage::to_json(Some(&storage::scan()), false, true)),
        Some("metrics") => println!("{}", serde_json::to_string_pretty(&sampler::one_shot()).unwrap()),
        Some("classify") => match args.next() {
            Some(key) => {
                let snap = snapshot::take();
                match snap.groups.iter().find(|g| g.key == key) {
                    Some(g) => {
                        println!("label: {:?}", g.trust);
                        println!("rules_fired: {:?}", g.rules_fired);
                        println!("allowed_actions: {:?}", classify::allowed_actions(g.trust, g));
                        if let Some(ev) = classify::evidence(&key) { println!("{}", serde_json::to_string_pretty(&ev).unwrap()); }
                    }
                    None => eprintln!("no such group: {key}"),
                }
            }
            None => eprintln!("usage: monitor-lizard classify <group-key>"),
        },
        Some("llm-classify") => match (args.next(), args.next()) {
            (Some(key), port) => {
                let port: u16 = port.and_then(|p| p.parse().ok()).unwrap_or(8090);
                match classify::evidence(&key) {
                    Some(ev) => match llm::classify(&format!("http://127.0.0.1:{port}"), &ev) {
                        Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap()),
                        Err(llm::LlmError::Unavailable(reason)) => eprintln!("llm call failed: server unavailable: {reason}"),
                        Err(llm::LlmError::Invalid(reason)) => eprintln!("llm call failed: invalid reply: {reason}"),
                        Err(llm::LlmError::Timeout) => eprintln!("llm call failed: timed out"),
                    },
                    None => eprintln!("no such group: {key}"),
                }
            }
            (None, _) => eprintln!("usage: monitor-lizard llm-classify <group-key> [port=8090]"),
        },
        Some("json") => println!("{}", serde_json::to_string_pretty(&snapshot::take()).unwrap()),
        None | Some("snapshot") => print_snapshot(),
        Some("help") => println!("{}", guide::render_help(&args.collect::<Vec<_>>().join(" "))),
        Some("model") => match args.next().as_deref() {
            Some("install") => model_install(args),
            Some("status") => model_status(args),
            _ => eprintln!("usage: monitor-lizard model install [--model qwen3-4b|qwen3-1.7b] [--yes] | model status [--model qwen3-4b|qwen3-1.7b]"),
        },
        Some("agent") => match args.next().as_deref() {
            Some(word @ ("on" | "shadow" | "off")) => {
                let mode = settings::AgentMode::parse(word).expect("matched above");
                match settings::set_agent_mode(mode) {
                    Ok(()) => println!("{}", match mode {
                        settings::AgentMode::On => "agent: on (a running `serve` picks this up within a second or two; the Apps drawer shows the suggestion)",
                        settings::AgentMode::Shadow => "agent: shadow (the model runs and each verdict is recorded; the dashboard shows nothing)",
                        settings::AgentMode::Off => "agent: off (the kill switch; a running `serve` stops llama-server within a second or two)",
                    }),
                    Err(e) => { eprintln!("could not update settings: {e}"); std::process::exit(1); }
                }
            }
            _ => eprintln!("usage: monitor-lizard agent on | agent shadow | agent off"),
        },
        Some("approve") => for arg in args {
            let (kind, key) = baseline::parse_entry(&arg);
            match baseline::change(kind, key, true) {
                Ok(true) => println!("marked {} {key} as expected", kind.name()),
                Ok(false) => println!("{} {key} was already expected", kind.name()),
                Err(e) => { eprintln!("could not mark {arg}: {e}"); std::process::exit(1); }
            }
        },
        _ => eprintln!("{}", guide::usage()),
    }
}

struct ModelArgs {
    model: String,
    yes: bool,
}

/// `--model <id>` and, only where `allow_yes`, `--yes`, in any order; any other flag is a usage
/// error, so a typo (`--Model`, `--models`) is caught rather than silently ignored. One pass over
/// `args`, so `--model`/`--yes` are each read once, not scanned for separately by two callers.
fn parse_model_args(mut args: impl Iterator<Item = String>, allow_yes: bool) -> Option<ModelArgs> {
    let mut model = model_store::DEFAULT_MODEL.to_string();
    let mut yes = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--model" => match args.next() {
                Some(v) => model = v,
                None => { eprintln!("--model needs a value"); return None; }
            },
            "--yes" if allow_yes => yes = true,
            other => { eprintln!("unknown flag: {other}"); return None; }
        }
    }
    Some(ModelArgs { model, yes })
}

/// Exits with a usage error (listing the known ids) rather than returning `None`, since every
/// caller would otherwise repeat the same message and exit call.
fn resolve_model(id: &str) -> &'static model_store::ModelEntry {
    model_store::find(id).unwrap_or_else(|| {
        eprintln!("unknown model: {id} (known: {})", model_store::MANIFEST.iter().map(|m| m.id).collect::<Vec<_>>().join(", "));
        std::process::exit(1);
    })
}

fn format_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1000.0 && i < UNITS.len() - 1 { v /= 1000.0; i += 1; }
    if i == 0 { format!("{n} B") } else { format!("{v:.1} {}", UNITS[i]) }
}

fn describe_install_error(e: &model_store::InstallError) -> String {
    match e {
        model_store::InstallError::DiskSpace { free_bytes, min_bytes } => format!("only {} free, need at least {}", format_bytes(*free_bytes), format_bytes(*min_bytes)),
        model_store::InstallError::Network(why) => format!("network error: {why}"),
        model_store::InstallError::Status(code) => format!("the Hub replied with status {code}"),
        model_store::InstallError::HashMismatch { expected, got } => format!("hash mismatch: expected {expected}, got {got} (the download was deleted)"),
        model_store::InstallError::Io(why) => format!("I/O error: {why}"),
    }
}

fn model_install(args: impl Iterator<Item = String>) {
    let Some(ModelArgs { model: model_id, yes }) = parse_model_args(args, true) else { std::process::exit(1) };
    let entry = resolve_model(&model_id);
    // The licence leads, per the todo's own Goal: a legal gate the owner reads before committing
    // to a multi-gigabyte download, not an afterthought at the bottom of the manifest.
    println!("licence: {}", entry.licence);
    println!("model:   {}", entry.id);
    println!("repo:    {} @ {}", entry.repo, entry.commit);
    println!("file:    {}", entry.file_name);
    println!("size:    {}", format_bytes(entry.size_bytes));
    if !yes {
        println!("\nNothing downloaded. Pass --yes to download and verify this file.");
        return;
    }
    match model_store::install(entry) {
        Ok(model_store::InstallOutcome::AlreadyInstalled) => println!("\nalready installed and verified at {}", model_store::install_path(entry).display()),
        Ok(model_store::InstallOutcome::Installed) => println!("\ninstalled and verified at {}", model_store::install_path(entry).display()),
        Err(e) => { eprintln!("\ninstall failed: {}", describe_install_error(&e)); std::process::exit(1); }
    }
}

fn model_status(args: impl Iterator<Item = String>) {
    let Some(ModelArgs { model: model_id, .. }) = parse_model_args(args, false) else { std::process::exit(1) };
    let entry = resolve_model(&model_id);
    match model_store::status(entry) {
        model_store::StatusReport::NotInstalled => println!("{}: not installed", entry.id),
        model_store::StatusReport::Verified(hash) => println!("{}: installed and verified, sha256={hash}", entry.id),
        model_store::StatusReport::HashMismatch { expected, got } => {
            println!("{}: installed but the hash does not match (expected {expected}, got {got})", entry.id);
            std::process::exit(1);
        }
    }
}

fn print_snapshot() {
    let s = snapshot::take();
    let t = &s.totals;
    println!("{} processes, {} sockets ({} listening, {} outbound), {} groups, {} findings", t.processes, t.connections, t.listening, t.outbound, t.groups, t.findings);
    let mut cats: BTreeMap<&str, Vec<&model::Group>> = BTreeMap::new();
    for g in &s.groups { cats.entry(&g.category).or_default().push(g); }
    for (cat, gs) in cats {
        println!("\n=== {cat} ({} groups, {} procs) ===", gs.len(), gs.iter().map(|g| g.procs.len()).sum::<usize>());
        let (busy, idle): (Vec<_>, Vec<_>) = gs.into_iter().partition(|g| !g.conns.is_empty());
        for g in &busy {
            let out: Vec<_> = g.conns.iter().filter(|c| c.direction == model::Direction::Outbound).collect();
            let hosts: std::collections::BTreeSet<_> = out.iter().filter_map(|c| c.remote_addr.as_deref()).collect();
            let mut lp: Vec<_> = g.conns.iter().filter(|c| c.direction == model::Direction::Listen).map(|c| format!("{}{}", c.local_port, if c.exposed { "*" } else { "" })).collect();
            lp.sort(); lp.dedup();
            let sig = match &g.signing { Some(s) if s.apple => "apple".into(), Some(s) if s.signed => format!("signed:{}", s.team.clone().unwrap_or_default()), Some(_) => "UNSIGNED".into(), None => String::new() };
            println!("{:<30} procs={:<3} out={:<3}-> {:<2} hosts  listen={} {sig}", g.label, g.procs.len(), out.len(), hosts.len(), if lp.is_empty() { "-".into() } else { lp.join(",") });
        }
        if !idle.is_empty() {
            println!("  + {} groups with no sockets ({} procs): {}...", idle.len(), idle.iter().map(|g| g.procs.len()).sum::<usize>(), idle.iter().take(8).map(|g| g.label.as_str()).collect::<Vec<_>>().join(", "));
        }
    }
    println!("\n=== Findings ===");
    for f in &s.findings { println!("[{:?}] {}: {}: {}", f.severity, f.group, f.rule, f.detail); }
}
