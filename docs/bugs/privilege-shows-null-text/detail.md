# The text "null" appears above "sudo / su launches (live capture)"

Resolution: [resolution.md](resolution.md)

## What we saw

On the Privilege page the word "null" sits above the heading "sudo / su launches (live capture)". It shows only when there are no failed sudo authorizations in the last two hours.

## How to reproduce

1. Build and run the page on a Mac with no failed sudo authorizations in the last two hours (2026-11-26: `target/release/sys-monitor serve 4961`).
2. Open `http://127.0.0.1:4961/#priv`.
3. Look above the heading. Reproduced in Chrome on 2026-11-26.

## Measurements

- In the real page, `S.priv` held 0 failed sudo entries. The only text node directly inside `#content` on the Privilege page was `"null"`.
- Rendering each of the eight views in turn and listing the text nodes directly inside `#content` found text only on Privilege. The other seven views do not return a nullish top-level item with the data present then.

## Cause

Two definitions of "nothing to show" disagree. `priv()` returns an array whose first item is `failed ? h('section', ...) : null`. The `h()` helper drops nullish children, so a `null` there is fine. `render()` instead passes the array to `Element.replaceChildren(...priv())`, and that method writes a `null` argument as the text "null". Every view goes through that call, so any view that lists a conditional section the same way would show the same text.
