# Bugs

One folder per diagnosed bug, named for the **symptom** (`viewer-blurry-at-zoom`), not the cause. A bug fix needs no todo: this folder is the record. Method: [guidelines/bug-triage.md](../guidelines/bug-triage.md).

Each folder holds two files that link to each other. Copy from [TEMPLATE-detail.md](TEMPLATE-detail.md) and [TEMPLATE-resolution.md](TEMPLATE-resolution.md). Add a row below in the same PR.

| Bug | Symptom | Cause | Fixed in |
|---|---|---|---|
| [stray-quote-bracket-top-left](stray-quote-bracket-top-left/detail.md) | A stray `">` at the top left of every page | Unescaped double quotes inside the favicon data URI ended its attribute early | [resolution](stray-quote-bracket-top-left/resolution.md) |
| [privilege-shows-null-text](privilege-shows-null-text/detail.md) | The text "null" above "sudo / su launches (live capture)" on the Privilege page | `render()` passed a `null` view item to `replaceChildren`, which writes it as text | [resolution](privilege-shows-null-text/resolution.md) |
