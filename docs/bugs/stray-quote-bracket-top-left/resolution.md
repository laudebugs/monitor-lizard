# Resolution: a stray `">` shows at the top left of every page

Detail: [detail.md](detail.md)

## The fix

The favicon's `href` now holds a percent-encoded SVG: inner double quotes became single quotes, and `<` and `>` became `%3C` and `%3E`. `#` was already `%23`. No behavior changed besides the page text.

## How it was checked

| Check | Before | After |
|---|---|---|
| Text outside tags in the head part, via Python `html.parser` | `'">'` present | absent |
| `cargo test favicon_href_stays_inside_its_attribute` | fails when run on the old page: `raw angle bracket in href: data:image/svg+xml,<svg xmlns=` | passes |
| Looked at in a real browser | not done | not done: pending, the browser extension was not connected |

Regression test: `server::tests::favicon_href_stays_inside_its_attribute` in `src/server.rs`. It reads the embedded page and takes the value of the icon link's `href` up to the first double quote. It checks that the value has no raw angle brackets and ends with the encoded closing `</svg>`, and that the end of the tag follows it.

## Left for later

Someone should open the page in a real browser and confirm both the missing stray text and that the tab icon still shows (pending, per `AGENTS.md` rule 7).
