import type { Proc } from "../types.ts";
import { sh } from "./sh.ts";

export function parsePs(out: string): Proc[] {
  const procs: Proc[] = [];
  for (const line of out.split("\n")) {
    const m = line.trim().match(/^(\d+)\s+(\d+)\s+(\S+)\s+([\d.]+)\s+(\d+)\s+(.+)$/);
    if (!m) continue;
    const command = m[6];
    // exe = path up to ".app/Contents/MacOS/x" or first token that looks like a path
    const exe = command.match(/^(.*?\.app\/Contents\/MacOS\/.+?)(?= -|$)/)?.[1] ?? command.split(" -")[0].split(" ")[0];
    procs.push({ pid: +m[1], ppid: +m[2], user: m[3], cpu: +m[4], rssKb: +m[5], command, exe });
  }
  return procs;
}
export const collectProcesses = async () =>
  parsePs(await sh("ps", ["-axww", "-o", "pid=,ppid=,user=,pcpu=,rss=,command="]));
