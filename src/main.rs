mod baseline; mod collect; mod group; mod model; mod rules; mod server; mod snapshot;
use std::collections::BTreeMap;

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("serve") => server::serve(args.next().and_then(|p| p.parse().ok()).unwrap_or(4747)),
        Some("json") => println!("{}", serde_json::to_string_pretty(&snapshot::take()).unwrap()),
        None | Some("snapshot") => print_snapshot(),
        Some("approve") => for k in args { baseline::approve(&k).expect("write baseline"); println!("approved {k}"); },
        _ => eprintln!("usage: ownwatch [snapshot | json | serve [port] | approve <group-key>...]"),
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
