# The text "null" appears above "sudo / su launches (live capture)"

Resolution: [resolution.md](resolution.md)

## What we saw

On the Privilege page the word "null" sits above the heading "sudo / su launches (live capture)". It shows only when there are no failed sudo authorizations in the last two hours.

## How to reproduce

1. Build the page on a Mac with no failed sudo authorizations in the last two hours and serve it (`target/release/sys-monitor serve 4961`).
2. Open `http://127.0.0.1:4961/#priv`.
3. Look above the heading.

Reproduced on 2026-11-26 in Chrome, in a tab opened for the check, on the build of `main` at `b55eada`. The page was in the dark theme and showed `null` above the heading. After the theme was switched to light it showed the same text. `S.priv` held 0 failed sudo entries.

## Measurements

- In the page, the only text node directly inside `#content` on the Privilege page was `"null"`.
- With the same data, each of the eight views was rendered in turn and the text nodes directly inside `#content` were counted. Only the Privilege page had one. The other seven views have no nullish top-level item with the data loaded then; a different data set could still produce one, so the fix does not rely on that.

## Cause

Two definitions of "nothing to show" disagree. `priv()` returns an array whose first item is `failed ? h('section', ...) : null`. The `h()` helper drops nullish children, so a `null` there is fine. `render()` instead passes the array to `Element.replaceChildren(...priv())`, and that method writes a `null` argument as the text "null". Every view goes through that call, so any view that lists a conditional section the same way would show the same text.
