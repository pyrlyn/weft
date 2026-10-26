# Evaluation

Translations: [Russian](../docs/ru/evaluation.md), [Ukrainian](../docs/uk/evaluation.md).

Report for T8, first written 2026-10-09, revised 2026-10-26 with the final runs. By the creator's scope change, the one local model, `prism-ml/bonsai-27b` in LM Studio, measured against the baselines, is enough; the Claude runs are no longer required. The final runs miss the first-try bar by 0.6 points, so T8 stays open.

## Criteria

Weft must reach at least 95% validity on the first try and at least 99% after one repair cycle. Success, output size and the gap to HTML, JSX and A2UI inform the continue/stop recommendation. Raw results must be in the repository. They are, under `bench/results/`.

## Final runs (2026-10-25/26)

LM Studio, `prism-ml/bonsai-27b` (32k context, 4 parallel slots), the current harness (Rust core, T25 checkers, one repair cycle, replies saved), commit `d0aa14cf`, three samples, every task and format, no readback turn. The first full edit run stopped after 57 of 432 samples when LM Studio's model process crashed (`bench/results/edit-2026-10-25T17-48-11-985Z.*`), so both runs were made one screen at a time: twelve edit files (`edit-2026-10-25T19-49-32-893Z` to `edit-2026-10-26T04-39-24-874Z`) and twelve read files (`read-2026-10-26T05-12-18-880Z` to `read-2026-10-26T06-35-12-873Z`), with the same tasks, prompts and checks as a single run; no screen needed a retry. Logs: `bench/results/edit-bonsai-t8.log`, `bench/results/read-bonsai-t8.log`. One history row per file in `test.md`. The tables below combine the files with the harness's own `summarize`.

Edit, 36 tasks, 432 samples:

| Format | Valid | Valid after repair | Success | Success after repair | Mean output tokens |
| --- | ---: | ---: | ---: | ---: | ---: |
| Weft | 94.4% | 100.0% | 87.0% (86.1–88.9%) | 91.7% (88.9–94.4%) | 247 |
| HTML | 99.1% (97.2–100%) | 100.0% | 98.1% (94.4–100%) | 99.1% (97.2–100%) | 254 |
| JSX | 100.0% | 100.0% | 86.1% (83.3–88.9%) | 86.1% (83.3–88.9%) | 293 |
| A2UI | 100.0% | 100.0% | 93.5% (88.9–97.2%) | 93.5% (88.9–97.2%) | 831 |

Read, 24 questions, 288 samples:

| Format | Answered right | Mean output tokens |
| --- | ---: | ---: |
| Weft | 98.6% (95.8–100%) | 8 |
| HTML | 100.0% | 8 |
| JSX | 100.0% | 9 |
| A2UI | 94.4% (91.7–95.8%) | 8 |

Against the bar: Weft first-try validity is 94.4% (102 of 108), below 95%; validity after one repair is 100%, above 99%.

### Weft failures

- **Invalid on the first try (6 of 108), all fixed by the one repair.** Three are W116, a brace value that is not one whole reference: an unclosed `value="{$.phone"` (`signup.e1`), a padded `disabled="{ $todo.locked }"` (`todo-list.e2`) and `disabled="{!.signingOut}"` (`menu.e2`). One is W305, `disabled="{$signingOut}"`, a data path written as a loop variable (`menu.e2`). Two are on `todo-list.e3`: an event named `change` where the checkbox's event is `on-change` (W402), and a stray `; change` in the attribute list (W101, W105, W107).
- **Valid but wrong after repair (9 of 108), one class.** Every miss is a `disabled` binding written with a negation the task does not ask for: `{!$.busy}`, `{!$.saving}`, `{!$todo.locked}`, `{!$.signingOut}` (`login.e2` ×2, `settings.e3`, `confirm-dialog.e3`, `todo-list.e2`, `profile.e3` ×2, `menu.e2` ×2). Four of the six screens already carry a negated `disabled` (`{!$.email}`, `{!$.dirty}`, `{!$.draft}`, `{!$.isOwner}`), and the model carries the `!` over. Repair cannot see it, since the markup is valid. This is the class T28's readback turn targets; on the login trial (`test.md`, 2026-10-09) the readback fixed it.
- **Read:** one wrong answer in 72 (`menu.q1`, sample 1).

### Baseline failures, in short

- **JSX** is always valid and loses on success: 15 misses, on the same `disabled` edits as Weft (`login.e2` ×3, `settings.e3` ×2, `profile.e3`) and on placement edits (`search-results.e2` ×3, `error-state.e1` ×2, `error-state.e3` ×3, `login.e1`).
- **A2UI** is always valid and the most verbose (831 output tokens against 247 for Weft); 7 edit misses (`error-state.e1` ×3, `search-results.e1` ×2, `search-results.e3`, `profile.e1`) and 4 wrong answers (`settings.q2` ×3, `login.q1`).
- **HTML** is the strongest baseline on this model: one edit miss (`confirm-dialog.e3`) and every answer right.

## Earlier runs

| Run | Samples | Replies saved | Weft valid / after repair / success | Notes |
| --- | ---: | --- | --- | --- |
| 2026-10-03 edit, all 36 tasks, Sonnet 5.5, Opus 5.5, Haiku 4.5 | 1 | no | 100% / 100% / 100% | Before T25. Read run stopped after 39 of 288 requests (credit exhausted); nothing saved. |
| 2026-10-04 Bonsai 27B, login only | 1 | yes | edit 100% / 100% / 66.7%; read 100% answered | Smoke. The edit miss is `login.e2`. |

The October 3 edit file is `bench/results/edit-2026-10-03T20-19-03-199Z.json` (432 results: 3 models × 36 tasks × 4 formats). Every Weft result is valid and successful. No result in that file was repaired: every miss was a valid document that failed an assertion, so the repair prompt never fired.

Mean output tokens on that run, from the summary:

| Model | Weft | HTML | JSX | A2UI |
| --- | ---: | ---: | ---: | ---: |
| claude-sonnet-5-5 | 366 | 361 | 426 | 1004 |
| claude-opus-5-5 | 346 | 334 | 409 | 977 |
| claude-haiku-4-5-20251001 | 264 | 276 | 321 | 863 |

### Failures in the earlier runs

Eight results failed, all of them baselines, all of them valid:

- `login.e1` JSX, Sonnet and Opus: the Remember me checkbox was not between the password field and Sign in.
- `search-results.e2` JSX (all three models) and HTML (Haiku): the empty-state copy was lost or not placed after "No results found".
- `error-state.e1` JSX and A2UI, Haiku: the Reload page button was not placed after Retry.

Weft had no failure in that file. The Bonsai smoke run did: `login.e2` changed `disabled="{!$.email}"` to `disabled="{!$.busy}"` and kept the `!`, so the button is disabled while `$.busy` is falsy. HTML and JSX on the same run got `{$.busy}` / `data.busy`. The markup validates. The repair cycle cannot see it. T28's readback turn is the follow-up for that class of miss; it is in the harness (`--readback`) and is not in any saved run yet.

## Recommendation

Continue, and keep T8 open.

On the final runs, Weft meets the repair bar (100% valid after one repair, bar 99%) and misses the first-try bar by 0.6 points (94.4%, bar 95%): six of 108 replies, each a one-attribute slip that the diagnostic named and one repair fixed. Its replies are the shortest of the four formats (247 output tokens; HTML 254, JSX 293, A2UI 831). Edit success after repair, 91.7%, is above JSX (86.1%), near A2UI (93.5%) and below HTML (99.1%), and every Weft miss is one class: a negation carried into a `disabled` binding, which JSX misses on the same tasks. Reading is at 98.6%. Nothing here is a reason to stop.

It is not a pass either: by the numeric bar, T8 does not close on this run. What would move the numbers, for the creator to choose:

- An edit run with `--readback` (T28), which targets the one class of valid-but-wrong edits; it changes success, not first-try validity.
- The first-try misses are malformed references (W116, W305) and an event name (W402). Whether the primer should say more about them is a method change, recorded in `test.md` before any rerun.
- A plain rerun is unlikely to cross the bar: each of the three samples sits at exactly 94.4% (two invalid replies out of 36).

The Claude runs of the original plan are not part of the decision any more (scope change); the 2026-10-03 one-sample run on three Claude models (100% / 100%) stays below as an earlier measurement.

## Constrained generation (T12.3)

The question: does canonical JSON held to the catalog's document schema beat free markup on validity and edit success, and what does it cost in output tokens? Three formats answer it: `weft` (markup), `weft-json` (free canonical JSON) and `weft-json-constrained` (the same, decoded under the schema). The method is in [`test.md`](../test.md).

Status, 2026-10-12: the harness is covered by a mocked-provider test, and one local trial has been run (below). Anthropic cannot take the schema.

- **Anthropic: unsupported for the constrained format.** Its structured outputs reject recursive schemas, numeric bounds, more than 16 union-typed and more than 24 optional parameters per request. The core document schema has 36 self-reaching `$defs`, 6 bounds, 132 `anyOf` and 416 optional properties (`research.md` §20), so `run.ts` refuses the format on Anthropic before any call. Markup against free JSON still runs there.
- **LM Studio:** takes the schema as `response_format` and holds the reply to it (below).

### Trial: Bonsai 27B, login screen

LM Studio, `prism-ml/bonsai-27b`, the three login edit tasks, three samples each, commit `7caefe55`; raw results in `bench/results/edit-2026-10-12T18-31-58-922Z.*`, history row in `test.md`.

| Format | Valid | Valid after repair | Success | Success after repair | Mean output tokens |
| --- | ---: | ---: | ---: | ---: | ---: |
| `weft` | 88.9% | 88.9% | 55.6% | 55.6% | 240 |
| `weft-json` | 100.0% | 100.0% | 100.0% | 100.0% | 648 |
| `weft-json-constrained` | 66.7% | 66.7% | 33.3% | 33.3% | 380 |

- **The schema was accepted and obeyed.** Every constrained reply was JSON that passed the schema (checked afterwards with Ajv 8.20, draft 2020-12); nothing was rejected or partly ignored. So did every free JSON reply. The invalid ones (all three `login.e2` samples) break rules no JSON Schema states: an id used twice (W301) and a submit button outside its form (W313). Repair did not fix them.
- **The grammar admits nonsense.** All three `login.e1` constrained samples dropped the checkbox's `$.remember` binding and filled it with schema-legal but meaningless props (`perspective`, `rotate-x`); valid, but the edit failed.
- **Free JSON won this trial.** Markup lost on one invalid event name in `login.e1` and on a `login.e2` answer that did not bind `disabled` to `$.busy` in any sample.
- **Confounds.** Under the schema the model writes no thinking (the grammar holds it from the first token), while the free formats think first; their reasoning tokens are excluded from the means. The free JSON was pretty-printed and the constrained JSON compact, which accounts for most of the token gap between the two.
- **Reading.** Nine samples per format on one screen and one local model. Constrained decoding guarantees shape, not validity, and here it cost success. It is not evidence for constrained output as the default path; a broader run, on a provider that takes the schema, would be needed to say more.
