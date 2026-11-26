# Resolution: the text "null" above "sudo / su launches (live capture)"

Detail: [detail.md](detail.md)

## The fix

One function, `nodesOf`, now decides what is dropped: `null`, `undefined` and `false`, after flattening nested arrays. `h()` uses it for its children and `render()` uses it for each view's items before `replaceChildren`. The rule has one definition, so the two places cannot disagree again, and `priv()` keeps its conditional section. Every view is covered because they all go through `render()`.

## How it was checked

| Check | Before | After |
|---|---|---|
| `node --test tests/web/view-items.test.mjs` on the old page | fails: the `view-items` block does not exist | passes (6 tests) |
| Same test with the filter removed from `nodesOf` | not applicable | 2 tests fail (null items and nested arrays) |
| Same test with one view written as `c.replaceChildren(...(historyView()))` | not applicable | the bypass test fails |
| Chrome, own tab, port 4961, `#priv`, dark theme, 0 failed entries | first child of `#content` is the text `"null"` | first child is the `H2` heading |
| Same page, light theme | the text `"null"` shows above the heading | first child is the `H2` heading, no text nodes, screenshot checked |
| All eight views rendered in turn, text nodes directly inside `#content` | Privilege: 1 | 0 in every view |
| One injected failed sudo entry (run on the first fix, before `nodesOf` replaced `viewItems`) | not run | the warning section is the first child, no text nodes |

Regression test: `tests/web/view-items.test.mjs`. It lifts the `view-items` block out of `web/index.html`, as the URL-state tests do with theirs, and checks that `h()` and every view call in `render()` use `nodesOf`.

## Left for later

The 400 px layout was not checked: the change touches no layout. The injected-entry check was not repeated after the rename from `viewItems` to `nodesOf`.
