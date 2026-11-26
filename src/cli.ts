import { takeSnapshot } from "./snapshot.ts";
const s = await takeSnapshot();
console.log(s.totals);
const cats = Map.groupBy(s.groups, (g) => g.category);
for (const [cat, gs] of cats) {
  console.log(`\n=== ${cat} (${gs.length} groups, ${gs.reduce((n, g) => n + g.procs.length, 0)} procs) ===`);
  const busy = gs.filter((g) => g.conns.length);
  const idle = gs.filter((g) => !g.conns.length);
  for (const g of busy) {
    const out = g.conns.filter((c) => c.direction === "outbound");
    const lis = g.conns.filter((c) => c.direction === "listen");
    const remotes = new Set(out.map((c) => c.remoteAddr));
    const sig = g.signing ? (g.signing.apple ? "apple" : g.signing.signed ? `signed:${g.signing.team ?? "?"}` : "UNSIGNED") : "";
    console.log(`${g.label.padEnd(34)} procs=${String(g.procs.length).padStart(3)} out=${String(out.length).padStart(2)}→${remotes.size} hosts  listen=${lis.map((c) => c.localPort + (c.exposed ? "*" : "")).join(",") || "-"} ${sig}`);
  }
  if (idle.length) console.log(`  + ${idle.length} groups with no network sockets (${idle.reduce((n, g) => n + g.procs.length, 0)} procs): ${idle.slice(0, 8).map((g) => g.label).join(", ")}…`);
}
console.log("\n=== Findings ===");
for (const f of s.findings) console.log(`[${f.severity}] ${f.group}: ${f.rule}: ${f.detail}`);
