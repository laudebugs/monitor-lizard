import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { approve } from "./baseline.ts";
import { takeSnapshot } from "./snapshot.ts";

const PORT = Number(process.env.PORT ?? 4747);
const HOST = "127.0.0.1"; // never bind wider: this tool shows everything about the machine
createServer(async (req, res) => {
  try {
    const url = new URL(req.url ?? "/", `http://${HOST}`);
    if (url.pathname === "/api/snapshot") {
      res.setHeader("content-type", "application/json");
      return void res.end(JSON.stringify(await takeSnapshot()));
    }
    if (url.pathname === "/api/approve" && req.method === "POST") {
      // Reject cross-origin writes (DNS-rebinding / CSRF guard).
      if (req.headers.host !== `${HOST}:${PORT}` || (req.headers.origin && req.headers.origin !== `http://${HOST}:${PORT}`)) { res.statusCode = 403; return void res.end(); }
      await approve(url.searchParams.get("key") ?? "");
      return void res.end("ok");
    }
    res.setHeader("content-type", "text/html");
    res.end(await readFile(new URL("../public/index.html", import.meta.url)));
  } catch (e) { res.statusCode = 500; res.end(String(e)); }
}).listen(PORT, HOST, () => console.log(`ownwatch → http://${HOST}:${PORT}`));
