mod actions; mod baseline; mod collect; mod group; mod history; mod model; mod persistence; mod privilege; mod rules; mod server; mod snapshot; mod watcher;
use std::collections::BTreeMap;

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("serve") => server::serve(args.next().and_then(|p| p.parse().ok()).unwrap_or(4747)),
        Some("privilege") => for e in privilege::recent(args.next().and_then(|m| m.parse().ok()).unwrap_or(60)) {
            println!("{} {} {} right={} {} parent={}", e.time, if e.ok { "OK  " } else { "FAIL" }, e.client, e.right, e.error.unwrap_or_default(), e.parent.unwrap_or("(exited)".into()));
        },
        Some("watch") => {
            let h = history::History::open().ok();
            println!("watching for sudo/su (Ctrl-C to stop)...");
            watcher::watch(|e| {
                println!("[{}] {} pid={} {}", e.ts, e.tool, e.pid, e.args);
                for (i, hop) in e.chain.iter().enumerate() { println!("  {}{} {} {}", "  ".repeat(i), if i == 0 { "parent" } else { "└" }, hop.pid, if hop.command.is_empty() { &hop.exe } else { &hop.command }); }
                if let Some(h) = &h { let _ = h.record_sudo(&e); }
            })
        }
        Some("sudo-log") => if let Ok(h) = history::History::open() {
            for e in h.recent_sudo(args.next().and_then(|n| n.parse().ok()).unwrap_or(20)).unwrap_or_default() {
                println!("[{}] {} pid={} {}\n   via {}", e.ts, e.tool, e.pid, e.args, e.chain.iter().map(|h| h.command.split_whitespace().next().unwrap_or(&h.exe).to_string()).collect::<Vec<_>>().join(" <- "));
            }
        },
        Some("persistence") => for i in persistence::take() {
            println!("{}{:<12} {:<7} {} -> {}", if i.is_new { "NEW " } else { "    " }, i.kind, i.scope, i.label, i.program);
        },
        Some("json") => println!("{}", serde_json::to_string_pretty(&snapshot::take()).unwrap()),
        None | Some("snapshot") => print_snapshot(),
        Some("approve") => for k in args { baseline::approve(&k).expect("write baseline"); println!("approved {k}"); },
        _ => eprintln!("usage: ownwatch [snapshot | persistence | watch | sudo-log [n] | json | privilege [minutes] | serve [port] | approve <group-key>...]"),
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
