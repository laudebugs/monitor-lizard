import type { Signing } from "../types.ts";
import { sh } from "./sh.ts";

const cache = new Map<string, Signing>();
export async function signingOf(path: string): Promise<Signing> {
  const hit = cache.get(path);
  if (hit) return hit;
  const out = await sh("codesign", ["-dv", "--verbose=2", path]);
  const authority = out.match(/^Authority=(.+)$/m)?.[1];
  const team = out.match(/^TeamIdentifier=(.+)$/m)?.[1];
  const signed = /Signature size|Authority=/.test(out) && !/not signed/i.test(out);
  const apple = /Software Signing|Apple Root CA/.test(out) && !team?.match(/^[A-Z0-9]{10}$/);
  const res: Signing = { signed, team: team === "not set" ? undefined : team, authority, apple };
  cache.set(path, res);
  return res;
}
