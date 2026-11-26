import type { Conn, Group, Proc } from "./types.ts";

/** Map an executable path to a stable group (app bundle, toolchain, system). */
export function groupOf(exe: string): { key: string; label: string; category: string } {
  const app = exe.match(/\/([^/]+)\.app\//);
  if (app && !exe.startsWith("/System/")) {
    const top = exe.match(/^(.*?\/[^/]+\.app)/)?.[1] ?? exe;
    return { key: top, label: app[1], category: exe.startsWith("/Applications") || exe.includes("/Applications/") ? "Apps" : "Apps (other)" };
  }
  if (exe.startsWith("/System/") || exe.startsWith("/usr/libexec") || exe.startsWith("/usr/sbin") || exe.startsWith("/sbin") || exe.startsWith("/usr/bin"))
    return { key: "apple:" + exe, label: exe.split("/").pop() ?? exe, category: "macOS system" };
  if (exe.includes("/.nvm/") || exe.includes("/node_modules/") || /\/(node|bun|deno)$/.test(exe)) return { key: "dev:node", label: "Node / JS runtimes", category: "Developer tools" };
  if (exe.includes("/.cargo/") || exe.includes("/.pyenv/") || exe.startsWith("/opt/homebrew") || exe.startsWith("/usr/local"))
    return { key: "dev:" + exe, label: exe.split("/").pop() ?? exe, category: "Developer tools" };
  if (exe.includes("/.local/share/claude") || /^\d+\.\d+\.\d+$/.test(exe.split("/").pop() ?? "")) return { key: "dev:claude-code", label: "Claude Code", category: "Developer tools" };
  return { key: "other:" + exe, label: exe.split("/").pop() ?? exe, category: "Unclassified" };
}

export function buildGroups(procs: Proc[], conns: Conn[], approved: Set<string>): Group[] {
  const byPid = new Map(procs.map((p) => [p.pid, p]));
  const groups = new Map<string, Group>();
  const get = (exe: string) => {
    const g = groupOf(exe);
    let grp = groups.get(g.key);
    if (!grp) groups.set(g.key, (grp = { ...g, procs: [], conns: [], approved: approved.has(g.key) }));
    return grp;
  };
  for (const p of procs) get(p.exe).procs.push(p);
  for (const c of conns) {
    const p = byPid.get(c.pid);
    if (p) get(p.exe).conns.push(c);
  }
  return [...groups.values()].sort((a, b) => b.conns.length - a.conns.length || b.procs.length - a.procs.length);
}
