import { mkdir, readFile, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";

const dir = join(homedir(), ".ownwatch");
const file = join(dir, "baseline.json");
export async function loadBaseline(): Promise<Set<string>> {
  try { return new Set(JSON.parse(await readFile(file, "utf8")).approved ?? []); } catch { return new Set(); }
}
export async function approve(key: string) {
  const s = await loadBaseline();
  s.add(key);
  await mkdir(dir, { recursive: true });
  await writeFile(file, JSON.stringify({ approved: [...s] }, null, 2));
}
