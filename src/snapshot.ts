import { loadBaseline } from "./baseline.ts";
import { collectConnections } from "./collect/connections.ts";
import { collectProcesses } from "./collect/processes.ts";
import { signingOf } from "./collect/signing.ts";
import { buildGroups } from "./group.ts";
import { evaluate } from "./rules.ts";
import type { Snapshot } from "./types.ts";

export async function takeSnapshot(): Promise<Snapshot> {
  const [procs, conns, approved] = await Promise.all([collectProcesses(), collectConnections(), loadBaseline()]);
  const groups = buildGroups(procs, conns, approved);
  // Only verify signatures for groups that touch the network (codesign is slow).
  await Promise.all(groups.filter((g) => g.conns.length && g.procs[0]?.exe.startsWith("/")).map(async (g) => { g.signing = await signingOf(g.key.endsWith(".app") ? g.key : g.procs[0].exe); }));
  const findings = evaluate(groups);
  return {
    at: new Date().toISOString(), groups, findings,
    totals: { processes: procs.length, connections: conns.length, groups: groups.length, listening: conns.filter((c) => c.direction === "listen").length, outbound: conns.filter((c) => c.direction === "outbound").length, findings: findings.length },
  };
}
