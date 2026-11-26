use crate::{baseline, snapshot};
use tiny_http::{Header, Method, Response, Server};

const PAGE: &str = include_str!("../web/index.html");

pub fn serve(port: u16) {
    // Bind loopback only: this tool exposes everything about the machine.
    let addr = format!("127.0.0.1:{port}");
    let server = Server::http(&addr).expect("bind failed");
    println!("ownwatch -> http://{addr}");
    let origin = format!("http://{addr}");
    for req in server.incoming_requests() {
        let url = req.url().to_string();
        let (path, query) = url.split_once('?').unwrap_or((&url, ""));
        let hdr = |n: &'static str| req.headers().iter().find(|h| h.field.equiv(n)).map(|h| h.value.to_string());
        let host_ok = hdr("Host").is_some_and(|h| h == addr || h == format!("localhost:{port}")); // DNS-rebinding guard
        let json = |b: String| Response::from_string(b).with_header(Header::from_bytes("content-type", "application/json").unwrap());
        let res = match (req.method(), path) {
            _ if !host_ok => Response::from_string("bad host").with_status_code(403).boxed(),
            (Method::Get, "/api/snapshot") => json(serde_json::to_string(&snapshot::take()).unwrap()).boxed(),
            (Method::Post, "/api/approve") => {
                let ok = hdr("Origin").is_none_or(|o| o == origin || o == format!("http://localhost:{port}"));
                let key = query.strip_prefix("key=").map(urldecode).unwrap_or_default();
                if ok && !key.is_empty() && baseline::approve(&key).is_ok() { Response::from_string("ok").boxed() }
                else { Response::from_string("forbidden").with_status_code(403).boxed() }
            }
            (Method::Get, "/") => Response::from_string(PAGE).with_header(Header::from_bytes("content-type", "text/html; charset=utf-8").unwrap()).boxed(),
            _ => Response::from_string("not found").with_status_code(404).boxed(),
        };
        let _ = req.respond(res);
    }
}

fn urldecode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap_or(b'?')); i += 3;
            }
            b'+' => { out.push(b' '); i += 1 }
            c => { out.push(c); i += 1 }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
