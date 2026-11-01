# Remaining plugin and configuration simplifications

Assessment after `24ea77c`. The framework, app form, and GUI phase module total
5,000 main-code lines; the CLI questionnaire adds 734. These are source budgets,
not estimates of removable code. The previous authoritative-tree experiment was
rejected because it replaced JSON editing with more identity and adapter machinery.

The next change should remove a responsibility or representation. Moving functions
to smaller files, adding a common solver, and caching existing work do not by
themselves establish a simplification. No implementation is proposed in this
document as already approved.

## 1. Stop predicting whether future choices can become compatible

**Assessment: strongest structural opportunity, with a real interaction change.**

### Current responsibility

`Draft::candidates` does more than check the current selection. `candidate`,
`Sources`, and `Reach` distinguish open fields, answered single-plugin fields,
top-level lists, nested axes, alternative providers, and cyclic requirements.
For a missing fact they recursively ask whether some provider could fit, including
whether that provider's own requirements could be satisfied in turn.

The same `Reach` machinery postpones issues in `issues_in`. Both the app and CLI
filter candidates, and the CLI uses `is_sole_supplier` to preselect dependencies.
Changing only the GUI's option filtering would leave most of this mechanism alive.

### Proposed contract

- A chooser lists registered plugins of its axis, excluding duplicate list items.
  Recommendations still put useful starting choices first.
- Selecting a plugin is allowed even if the current configuration is incompatible.
  The form shows the concrete problems; adding or saving the phase requires exact
  validation, as today.
- During incomplete configuration, an unanswered field may postpone a missing-fact
  issue if its axis could supply that fact. This is a shallow catalog lookup,
  without recursively proving a feasible completion. Conflicting facts and failed
  value conditions are still reported.
- Automatic unambiguous fieldless dependencies remain separate. Selecting a
  configurable dependency requires an explicit choice or an initial suggestion.

Example: selecting Buchholz without points leaves Buchholz selected and reports
that points are needed. If every points provider conflicts with another choice,
the operator sees those issues after selecting one; the chooser no longer proves
that impossibility in advance. An invalid phase still cannot be played.

### Intended deletion and replacement

Remove recursive completion prediction from both candidate selection and partial
issue reporting. Replace it with axis membership, duplicate checks, shallow
open-slot information, and validation of the actual answers. Review whether the
`Candidate` wrapper and sole-supplier preselection can disappear too. Do not count
all 948 draft/enumeration lines as savings: editing, defaults, plugin construction,
diagnostics, and implied providers remain necessary.

The CLI must be included. Without predictive filtering it may offer choices that
later need correction; its retry loop must not silently ignore a missing dependency
or get stuck asking the wrong field. Its current special handling of unprovided
facts and sole suppliers needs an explicit replacement. A public API compatibility
adapter also counts toward cost; retaining the old predictor behind it would
defeat the main deletion.

### Acceptance and stopping rule

Exercise nested plugins, alternative providers, value conditions, cyclic
requirements, loaded invalid configs, and the synchronous and browser CLI.
Complete-phase validation, replay, saved data, and third-party openness must stay
intact. Cycles of compatibility requirements are not automatically invalid;
calculator execution cycles are a separate constraint.

Proceed only if the more permissive interaction is acceptable. This should remove
recursive prediction, not replace it with a graph of suggested repairs. A UI that
offers disabled options with recursively computed explanations still needs the
predictor and is a different proposal.

## 2. Remove the GUI's second representation of set entries

**Assessment: likely a small, behavior-preserving improvement.**

The app already hands out `SetEntry::{Chosen, Implied, Offered}` through
`ListInput::set`. `UnorderedList` maps it into another owned `SetItem` / `SetState`
representation, adding translated text and a key, and then matches that state
again to render it.

Render the app's three cases directly. Prepare owned paths and field data only
where event closures need ownership. Compute translated text and keys during
rendering; do not introduce a replacement presentation enum or store Dioxus data
in the app. Delete `SetItem`, `SetState`, their conversion, and `SetItem::key` if
the resulting rendering reads more directly.

Keep stable checkbox order, distinct keys when an implied entry becomes chosen,
dependency explanations, editable child fields, and accessible names. The existing
toggle, implied-metric, focus, translation, and accessibility tests are the contract.

This touches only part of the 964-line GUI phase module. It is not a major
architectural reduction. Reject it if direct rendering produces larger repeated
match arms or harder closure ownership. A universal list component with switches
for ordered lists, sets, and single choices would likely be worse: their controls
and focus behavior differ.

## 3. Prepare one immutable inspection for a group of draft queries

**Assessment: plausible readability improvement; net simplification unproven.**

`default_value`, `candidates`, `issues`, and `build` independently construct the
schema/compatibility tree. A form view enumerates actual and implied answers and
queries defaults and candidates for multiple fields. The algorithms already take
context/tree arguments internally, but callers cannot express that several reads
belong to one inspection.

Prototype an immutable inspection borrowing a draft, offering enumeration,
defaults, candidates, and issues from one prepared tree. Its borrow must prevent
editing while it is used. It expires before the next edit: no cache invalidation,
stable node identities, interior mutability, or second authoritative answer model.
The actual draft and its implied copy need separate inspections; their answers
must not be mixed. Configured plugin declarations and custom serde rejection must
continue to be evaluated correctly.

The hoped-for deletion is repeated setup and scattered interpretation in readers,
not compatibility logic. Existing single-query APIs may remain thin wrappers,
whose cost must be included. Do not start by designing a general visitor or cursor
framework. First show that one real `PhaseForm::view` flow and its library queries
become easier to follow.

Keep this only if the combined library/app code is clearer. It may improve runtime
cost while adding source and lifecycle concepts; that would be a performance
proposal, not evidence of the substantial simplification being sought.

## 4. Replace implied-provider resolution with a simpler loop

**Assessment: do not pursue without a concrete deletion proof.**

`add_implied`, `first_chain`, `chain`, and `implied` look elaborate for adding a
calculator. But their contract covers unique providers, provider construction,
transitive dependencies, ambiguous destination fields, and all-or-nothing additions
that do not break existing value conditions. They also tolerate unrelated problems
elsewhere in a partially configured draft.

Appending one missing provider at a time can leave half a failing chain selected.
Appending everything to a copy and requiring the whole draft to validate would
block an independent valid chain because of an unrelated existing issue. Adding
rollback, provenance, and exception filtering can recreate the current algorithm
with more machinery. Sharing this with predictive completion would unite two
different questions: whether some future provider could fit, and whether a unique
concrete chain can safely be added now.

Removing implied providers altogether would delete more code, including UI state,
but make operators manage implementation dependencies such as win-draw-loss records.
That is a poor fit for presets and advanced views hiding unimportant settings.
Keep the current feature. Preserve its explicit pre-transition boundary so replay
never depends on which plugins happen to be linked later.

## What not to combine

- Do not fold numeric integer/decimal variants into an untyped number plus flags
  just to merge match arms. The GUI already shares `NumberField`; integer parsing,
  range rules, and keyboard choices still differ. Savings are likely small and
  weaker type invariants would be a regression.
- Do not merge schema, editable answers, translated form data, and GUI components
  into one model. They serve different boundaries; the earlier tree experiment
  demonstrated the cost of replacing those distinctions with handles and adapters.
- Do not add a generic recommendation-update engine for future visibility levels.
  The current section-wide state is intentionally simpler. Future hidden-field
  following needs concrete behavior examples before additional machinery.

## Suggested order

First settle the operator-facing examples for target 1; it is the only target here
with a strong argument for deleting a substantial mechanism. Independently, target
2 is a bounded cleanup worth trying. Target 3 needs a small prototype and honest
net accounting. Leave target 4 alone for now.

For every prototype, compare the final edit/read flow, deleted types and algorithms,
replacement concepts, and combined implementation across consumers. Use line counts
as supporting evidence, not the acceptance criterion. A rejected prototype is a
useful result; do not keep a more elaborate architecture to justify the experiment.
