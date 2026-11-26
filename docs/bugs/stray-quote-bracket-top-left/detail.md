# A stray `">` shows at the top left of every page

Resolution: [resolution.md](resolution.md)

## What we saw

A literal `">` in the top left corner of the dashboard, in a browser tab and in the Tauri spike's webview, on every page. Reported by the Tauri shell spike on 2026-11-26.

## How to reproduce

1. `cargo run --release -- serve`, open `http://127.0.0.1:4747/`.
2. Look at the top left of the page. Not reproduced in a real browser this session (the browser extension was not connected); see the resolution for what was checked.

## Measurements

- The favicon is one `<link rel="icon">` line in `web/index.html` whose `href` is a `data:` URI holding an SVG.
- The SVG's own attributes use double quotes (`xmlns="http://www.w3.org/2000/svg"`, `viewBox="0 0 64 64"`), and the SVG's angle brackets are raw.
- A tolerant parser (Python `html.parser`) lists the text outside tags in the head part of the file: `['(function(){var t=localStorage', 'Sys Monitor', '">']`. The last item is the stray text.

## Cause

Two definitions of one thing disagreeing: the `href` attribute is delimited by double quotes, and the SVG inside it also uses double quotes. The first inner quote (after `xmlns=`) ends the attribute. The HTML parsing rules then treat the SVG's child tags as page elements, which moves the parser out of the head, and the closing `">` of the original attribute becomes visible text. This is reasoned from the HTML parsing rules and matches the report, not observed in a browser here.
