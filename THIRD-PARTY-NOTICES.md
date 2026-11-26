# Third-party notices

## IBM Plex Sans and IBM Plex Mono

- **Files:** `web/fonts/IBMPlex{Sans,Mono}-{Regular,Medium,SemiBold}.woff2`
- **Source:** [google/fonts](https://github.com/google/fonts/tree/main/ofl) (`ibmplexsans/`, `ibmplexmono/`), upstream [IBM Plex](https://github.com/IBM/plex)
- **Licence:** SIL Open Font License, Version 1.1 — full text in `web/fonts/OFL.txt` (identical for both families as published)
- **Copyright:** © 2017 IBM Corp. with Reserved Font Name "Plex"
- **Modified:** Sans was instantiated from the upstream variable font (`wght`, `wdth` axes) at three static weights (400, 500, 600; `wdth=100`) with `fonttools varLib.instancer`; Mono's static Regular/Medium/SemiBold TTFs were used as published. Both were then subsetted to basic Latin plus common punctuation (`U+0020-007E, U+2018-201F, U+2026, U+2013-2014`) and compressed to WOFF2 with `fonttools`. Subsetting and format conversion for web delivery under the original family name is the same practice Google Fonts' own CDN uses for these families. Per OFL 1.1 section 3, these are "Modified Versions"; the Reserved Font Name "Plex" is not used to name or market this project, and the licence (including its copyright and permission notices) is carried along unmodified.
