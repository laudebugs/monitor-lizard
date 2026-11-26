export type Scope = "loopback" | "lan" | "public";
export type Direction = "listen" | "outbound" | "inbound" | "local";

export interface Proc {
  pid: number; ppid: number; user: string; cpu: number; rssKb: number; command: string; exe: string;
}
export interface Conn {
  pid: number; name: string; proto: "TCP" | "UDP"; state: string;
  localAddr: string; localPort: number; remoteAddr?: string; remotePort?: number;
  direction: Direction; scope: Scope; exposed: boolean; // exposed = listening on all interfaces
}
export interface Signing { signed: boolean; team?: string; authority?: string; apple: boolean }
export interface Group {
  key: string; label: string; category: string; procs: Proc[]; conns: Conn[];
  signing?: Signing; approved: boolean;
}
export type Severity = "high" | "medium" | "low";
export interface Finding { severity: Severity; group: string; rule: string; detail: string }
export interface Snapshot { at: string; groups: Group[]; findings: Finding[]; totals: Record<string, number> }
