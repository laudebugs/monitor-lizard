# Resolution: the text "null" above "sudo / su launches (live capture)"

Detail: [detail.md](detail.md)

## The fix

`render()` now passes each view's array through `viewItems`, which drops `null`, `undefined` and `false` before `replaceChildren`. The fix sits in the one place all views go through, so `priv()` keeps its conditional section and no view needs its own guard. The other views were read for top-level nullish items; none had one.

## How it was checked

| Check | Before | After |
|---|---|---|
| `node --test tests/web/view-items.test.mjs` on the old page | fails: the `view-items` block does not exist | passes (4 tests) |
| Same test with the filter replaced by a pass-through | fails: the stand-in element shows `"null"` | passes |
| Text nodes directly inside `#content` on `#priv`, in Chrome (dark and light) | `"null"` is the first child | none; the first child is the `H2` heading |
| Same page with one injected failed sudo entry | not run | the warning section is the first child, no text nodes |

Regression test: `tests/web/view-items.test.mjs`. It lifts the `view-items` block out of `web/index.html`, as the URL-state tests do with theirs. It also fails if a view in `render()` calls `replaceChildren` without `viewItems`.

## Left for later

Nothing. A new view needs to go through `viewItems` too; the last test fails if it does not.
