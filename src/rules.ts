import type { Finding, Group } from "./types.ts";

/** Each rule answers: "is this activity plausibly mine?" Keep them small and explainable. */
export function evaluate(groups: Group[]): Finding[] {
  const out: Finding[] = [];
  for (const g of groups) {
    const exposed = g.conns.filter((c) => c.exposed);
    if (exposed.length && !g.signing?.apple)
      out.push({ severity: "high", group: g.label, rule: "exposed-listener", detail: `Listening on all interfaces: ${[...new Set(exposed.map((c) => c.localPort))].join(", ")}` });
    else if (exposed.length)
      out.push({ severity: "low", group: g.label, rule: "exposed-listener-apple", detail: `Apple service reachable from LAN: ${[...new Set(exposed.map((c) => c.localPort))].join(", ")}` });
    const net = g.conns.some((c) => c.scope !== "loopback");
    if (net && g.signing && !g.signing.signed)
      out.push({ severity: "high", group: g.label, rule: "unsigned-network", detail: "Unsigned binary talking to the network" });
    if (net && !g.approved && !g.signing?.apple)
      out.push({ severity: "medium", group: g.label, rule: "unreviewed", detail: "Network activity from a group you haven't approved yet" });
    if (g.conns.some((c) => c.localPort === 9222 && c.direction === "listen"))
      out.push({ severity: "medium", group: g.label, rule: "debug-port", detail: "Remote-debugging port 9222 open: any local process can drive this browser" });
  }
  const rank = { high: 0, medium: 1, low: 2 };
  return out.sort((a, b) => rank[a.severity] - rank[b.severity]);
}
