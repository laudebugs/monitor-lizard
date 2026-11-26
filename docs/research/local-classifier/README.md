# A local model that classifies processes and suggests actions

As of November 2026. Gathered on 2026-11-26 by web research from model cards, crates.io and project docs; nothing here ran a model. Hardware figures are unverified. This report did not go through the five-step pipeline in [../README.md](../README.md); there is no fact-check log yet.

**Thesis:** a small local model can explain and rank actions, but it must not decide the label. Rules decide; the model suggests; a human approves.

## Models (verified from Hugging Face model cards)

| Model | Size | Licence | Notes |
|---|---|---|---|
| [Qwen/Qwen3-4B-GGUF](https://huggingface.co/Qwen/Qwen3-4B-GGUF) | 4.0B params; Q4_K_M 2.5 GB | Apache-2.0 | Best quality for its size among those checked. Default |
| [Qwen/Qwen3-1.7B-GGUF](https://huggingface.co/Qwen/Qwen3-1.7B-GGUF) | 1.7B; Q8_0 1.83 GB | Apache-2.0 | Fallback for 8 GB Macs |
| [microsoft/Phi-4-mini-instruct](https://huggingface.co/microsoft/Phi-4-mini-instruct) | 3.8B | MIT | Its card warns it can hallucinate names and URLs |
| [google/gemma-3-1b-it](https://huggingface.co/google/gemma-3-1b-it) | 1.0B | Gemma terms, gated | Login and licence acceptance block a first-run download |
| [MoritzLaurer/deberta-v3-large-zeroshot-v2.0](https://huggingface.co/MoritzLaurer/deberta-v3-large-zeroshot-v2.0) | 0.4B | MIT | Label only, no rationale, 512-token limit |
| [BAAI/bge-small-en-v1.5](https://huggingface.co/BAAI/bge-small-en-v1.5) | 33M | MIT | Embeddings for similarity to apps you marked; not a classifier |

None of the cards documents a native JSON mode: reliable JSON comes from the runtime's grammar constraint. No security-specific small model worth shipping was found.

## Runtimes

`llama-server` as a subprocess (crash isolation, unloads when killed, accepts a JSON schema or GBNF grammar) is the recommended runtime. `llama-cpp-2` 0.1.161 is pre-1.0. `mistralrs` 0.8.1 (JSON-schema support not verified), `candle` (no built-in constraint), `ort` 2.0.0-rc (encoders only), `hf-hub` 1.0.0 (downloads, does not pin SHA-256). llama.cpp's grammar converter silently skips unsupported schema features, so keep the schema flat. Ollama, LM Studio and MLX were not researched.

## Evidence on accuracy and injection

No benchmark of small models judging a process from path, signing team, parent and ports was found, so accuracy is unproven: measure it on a labelled set from this machine. LLM triage labels can be flipped through attacker-controlled log fields ([arXiv 2605.24421](https://www.alphaxiv.org/abs/2605.24421)); process names and command lines are equally attacker-controlled. See also the [OWASP prompt-injection cheat sheet](https://cheatsheetseries.owasp.org/cheatsheets/LLM_Prompt_Injection_Prevention_Cheat_Sheet.html).

## Safety recommendations (opinion)

Pin the Hugging Face commit SHA and verify SHA-256 after download and at load; accept only GGUF, safetensors or ONNX, never pickle ([HF docs](https://huggingface.co/docs/hub/security-pickle)); run inference in a subprocess with no network; cap disk near 3 GB with a delete button; contact the Hub only for the explicit download and support offline pre-seeding.

## Proposed schema

Input (every string escaped, truncated to 256 characters, in quoted fields only): `exe_path_class`, `signed`, `team_id`, `notarized`, `parent`, `listening_ports`, `remote_host_count`, `first_seen_age_h`, `persistence`, `rule_label`, `rules_fired`, `user_baseline`.

Output, enforced by grammar: `label` (benign, unknown, suspicious), `confidence` (low, medium, high), `actions` (a subset of approve, ignore, investigate, quit, force_quit, block_network), `rationale` (240 characters at most). Code overrides `label` with `rule_label` whenever the rule is stricter.

## UI

A label chip and confidence band (no percentages) in the Apps table, sortable by uncertainty; on attention cards the rule label, one AI rationale line and at most one suggested action. The drawer shows evidence, then rules fired, then the model text, with the raw name and command line in a code box. When rules and model disagree or confidence is low, show "Unsure, investigate" and no quit suggestion. "Mark as expected / suspicious" writes a local baseline. First run shows model, size, licence, commit and SHA-256 with "Skip, rules only" fully working.

## Not verified

RAM and latency on M-series Macs, GGUF sizes for Phi-4-mini, Gemma and DeBERTa, the Qwen3-1.7B Q4 size, GGUF parser vulnerabilities, Ollama, LM Studio and MLX, mistralrs JSON-schema support, the Hub's SHA-256 field name.
