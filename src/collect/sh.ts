import { execFile } from "node:child_process";
export const sh = (cmd: string, args: string[], timeout = 15000): Promise<string> =>
  new Promise((resolve) =>
    execFile(cmd, args, { maxBuffer: 64 << 20, timeout }, (_e, out, err) => resolve(String(out || err || ""))),
  );
