import type { Conn, Direction, Scope } from "../types.ts";
import { sh } from "./sh.ts";

export function scopeOf(addr?: string): Scope {
  if (!addr) return "loopback";
  const a = addr.replace(/^\[|\]$/g, "");
  if (a === "127.0.0.1" || a === "::1" || a === "localhost") return "loopback";
  if (/^(10\.|192\.168\.|169\.254\.|172\.(1[6-9]|2\d|3[01])\.)/.test(a) || /^(fe80|fc|fd)/i.test(a)) return "lan";
  return "public";
}
const splitHostPort = (s: string): [string, number] => {
  const i = s.lastIndexOf(":");
  return [s.slice(0, i), Number(s.slice(i + 1)) || 0];
};

/** Parses `lsof -nP -i -F pcPnTt` field output. */
export function parseLsof(out: string): Conn[] {
  const conns: Conn[] = [];
  let pid = 0, name = "", proto = "", state = "";
  const flush = (n: string) => {
    if (!n) return;
    const [l, r] = n.split("->");
    const [localAddr, localPort] = splitHostPort(l);
    const remote = r ? splitHostPort(r) : undefined;
    const isListen = state === "LISTEN" || (proto === "UDP" && !r);
    const scope = scopeOf(remote?.[0] ?? localAddr);
    const exposed = isListen && ["*", "0.0.0.0", "[::]"].includes(localAddr);
    const direction: Direction = isListen ? "listen" : scope === "loopback" ? "local" : "outbound";
    conns.push({
      pid, name, proto: proto as "TCP" | "UDP", state: state || (r ? "ESTABLISHED" : "BOUND"),
      localAddr, localPort, remoteAddr: remote?.[0], remotePort: remote?.[1], direction, scope, exposed,
    });
  };
  let pending = "";
  for (const line of out.split("\n")) {
    const tag = line[0], val = line.slice(1);
    if (tag === "p") { flush(pending); pending = ""; pid = +val; state = ""; }
    else if (tag === "c") name = val;
    else if (tag === "f") { flush(pending); pending = ""; state = ""; }
    else if (tag === "P") proto = val;
    else if (tag === "n") pending = val;
    else if (tag === "T" && val.startsWith("ST=")) state = val.slice(3);
  }
  flush(pending);
  return conns;
}
export const collectConnections = async () => parseLsof(await sh("lsof", ["-nP", "-i", "-F", "pcPnTt"]));
