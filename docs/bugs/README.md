# Bugs

One folder per diagnosed bug, named for the **symptom** (`viewer-blurry-at-zoom`), not the cause. A bug fix needs no todo: this folder is the record. Method: [guidelines/bug-triage.md](../guidelines/bug-triage.md).

Each folder holds two files that link to each other. Copy from [TEMPLATE-detail.md](TEMPLATE-detail.md) and [TEMPLATE-resolution.md](TEMPLATE-resolution.md). Add a row below in the same PR.

| Bug | Symptom | Cause | Fixed in |
|---|---|---|---|
| [stray-quote-bracket-top-left](stray-quote-bracket-top-left/detail.md) | A stray `">` at the top left of every page | Unescaped double quotes inside the favicon data URI ended its attribute early | [resolution](stray-quote-bracket-top-left/resolution.md) |
| [privilege-shows-null-text](privilege-shows-null-text/detail.md) | The text "null" above "sudo / su launches (live capture)" on the Privilege page | `render()` passed a `null` view item to `replaceChildren`, which writes it as text | [resolution](privilege-shows-null-text/resolution.md) |
| [network-table-columns-collapse-at-narrow-width](network-table-columns-collapse-at-narrow-width/detail.md) | At 400 px the Network table's first three columns are one character wide; Apps and Listeners clip columns at 795 px | Probably `.tr.ell` removing the row's minimum width; not confirmed | Unresolved |
