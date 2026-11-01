# Where a substantial simplification could come from

Investigation after `e659fd1`. This is an architectural assessment, not a claim
of measured savings from an implementation. Current `cargo warloc` totals are
19,913 main and 16,054 test lines, excluding comments and blanks.

## Finding

The strongest internal candidate is **changing the configuration editing model**,
not sharing another helper. `Draft` currently edits the JSON representation from
which plugins are deserialized. Several layers reconstruct the structure and
meaning of those answers. A schema-aware editing model could replace that cycle.

The other major concentration is **configuration policy**: predicting which
plugins might become valid after future answers, choosing implied providers, and
following recommendations without overwriting custom choices. Removing those
policies would simplify the system, but would change configuration behavior.
That is a separate decision, not a transparent implementation refactor.

Neither candidate has a proven net saving yet. I cannot justify promising a
thousand-line reduction while preserving every existing behavior.

## Why the first two changes were small

The traversal change introduced `FieldNode` snapshots. It did not change what
`Draft` owns: `answers: Config`, backed by JSON. It removed some repeated reading
code but introduced a shared representation and conversion. The result was eight
fewer main lines.

The session change removed duplicate ownership and orchestration. Both sessions
already delegated tournament progression to the same engine, so they were not two
independent implementations of the tournament rules. The result was 51 fewer main
lines. One owner of operations is useful, but the amount of duplicated algorithm
was much smaller than the size of the two session files suggested.

For a bigger reduction, the unit to remove needs to be a representation, a
mechanism, or a product requirement, rather than another forwarding method.

## The concentration, measured now

| Area | Main code lines |
| --- | ---: |
| Generic plugin framework and its procedural macros | 3,287 |
| App configuration form | 823 |
| GUI phase form, summary, and editing dialog | 917 |
| CLI questionnaire | 734 |
| **Combined configuration/extensibility boundary** | **5,761** |

These disjoint file/crate counts are about 29% of the reported main code.
They are investigation budgets, not removable code. They include necessary
registration, compatibility checks, range handling, rendering, and interaction.

The narrower boundary of `config.rs`, `schema.rs`, `schema/range.rs`,
`draft.rs`, `draft/fields.rs`, and the app's `form.rs` is 2,842 main lines.
Not all of that implements representation conversion either.

The tournament formats and metric implementations remain comparatively small.
Rewriting them would miss this concentration and risk the result-reporting path.

## Candidate 1: make fields the editing model

### What the code does today

The same configuration has several representations with different purposes:

1. `Schema` / `FieldKind`: what answers are allowed.
2. `Config` / `PluginConfig` / JSON `Value`: what is answered, using serde's
   adjacent `type` / `value` plugin representation.
3. `FieldNode` / `PluginNode`: snapshots reconstructing the chosen plugin tree.
4. `Tree` / `Question` / `Placed`: fields plus instantiated plugins and their
   declarations for defaults, compatibility, and diagnostics.
5. `FormField` / `Input` / `Chosen` / `Item`: the app's representation for the GUI.

Not every distinction is redundant: a compatibility declaration and a translated
hint serve different purposes. The problem is repeatedly recovering field
structure and identity from the serialized answers to construct these views.

For example, entering a number goes through:

- GUI input → `PhaseForm::enter`.
- Enumeration and `field_at` to recover the field's numeric range.
- Parsing, then `PhaseForm::change` to identify later fields that should follow.
- `Draft::set` → `locate` to interpret the path against schemas and JSON again.
- `FieldKind::walk`, copying the answers, `edit`, and a whole-schema check.
- Defaults and recommendation updates, which perform further draft queries.
- `PhaseForm::view`, rebuilding the nodes and converting them into form inputs.

This does not prove poor runtime performance. It does make a simple edit require
understanding multiple translations and validation boundaries.

### The alternative

Give the draft one authoritative, schema-aware field structure:

```text
Field
  metadata: name, label, description, bounds
  state: unanswered / typed scalar / chosen plugin / plugin list / explicit none
  children: the fields of the chosen plugins
```

The GUI renders that structure and retains field identities for later edits.
The CLI visits it in question order, discovering children after choosing their
plugin. Paths describe diagnostics and support existing path-based APIs at the
boundary; routine editing does not have to reinterpret serde objects.

JSON conversion remains necessary for loading configs, constructing actual
plugins through serde, and exporting configuration. Existing saved files and
plugin tags do not need to change.

This would **replace** the JSON-backed editing machinery and snapshot adapters.
Adding this model alongside `Config`, `FieldNode`, and `FormField` indefinitely
would repeat the mistake of adding another layer for a small local cleanup.

### What should actually disappear

- Schema reconstruction and JSON object navigation in ordinary edits.
- `locate` / `set_in` / `edit` as the internal edit mechanism.
- The separate construction of a field tree from JSON for each reader.
- The structural part of converting draft fields into app form inputs.
- Path-prefix searches used only to recover field identity or ancestry.

Compatibility evaluation, recommendation policy, plugin instantiation, translated
messages, numeric parsing, and serialization adapters still need implementations.
They cannot honestly be counted as automatic savings from a new field model.
Invalid text also needs a place to live independently of the last accepted value:
the current form retains rejected input and blocks submission without corrupting
the valid draft. A new model must preserve that distinction.

### The difficult contracts a prototype must preserve

- Unanswered differs from explicitly answered with `None` or JSON `null`.
- Plugins may nest; lists reject the same plugin twice regardless of its config.
- Fields may be optional, ordered lists, unordered sets, or numeric ranges.
- A plugin may reject an answer through custom serde deserialization, beyond
  what its field schema can express. Building fields directly into Rust structs
  must not silently bypass that validation.
- Runtime plugin state is excluded from editable configuration, while snapshots
  of running competitions still preserve it.
- A field's recommendations depend on configured sibling plugins and their
  parameters, not just their catalog entries.
- Editing one field follows later recommendations without changing earlier
  choices, and preserves custom choices as the current tests specify.
- Stale GUI callbacks must not edit another list item after a move or removal.
- A failed edit must leave the draft unchanged.
- Third-party plugins remain open; consumers gain no table of known plugins.
- Ownership choices must be checked for changes to thread-safety properties.
  Per-field `Rc<RefCell<_>>` is not automatically a simpler substitute for paths.

### A useful prototype, and a stopping rule

Port a complete vertical slice: load a real phase config, enumerate fields,
change a format and its scalar settings, configure points, move ranking items,
show invalid input and missing dependencies, then build and serialize it.
Exercise nested and optional fields with the plugin-factory fixtures too.

Use the existing behavior tests as the specification. Tests and consumers may
need new handles, but their expected choices, errors, and saved data must remain.
Measure the combined implementation across the library, app, GUI, and CLI.
Moved code and compatibility shims count toward its cost.

Keep it only if the old conversion/editing machinery can be deleted and the
normal read and edit flows become substantially easier to follow. If managing
handles, ownership, and adapters replaces one difficult mechanism with another,
discard the prototype. A shorter helper or a faster cache is not enough.

## Candidate 2: simplify the policies that require prediction

There are three different questions in the current implementation:

| Question | Relevant implementation |
| --- | --- |
| Is this completed configuration valid? | `compatibility::check`, final draft build |
| Could this candidate become valid after other choices? | `candidate`, `Sources`, `Reach` |
| What should be added or changed for the operator? | `chain`, `default_in`, form recommendation following |

The first is essential correctness. The latter two are configuration assistance.
Keeping that distinction is important: simplifying assistance need not weaken
validation or close the plugin system, but it does change what the UI offers.

### Example: choosing Buchholz

Buchholz needs points, which another calculator provides. The candidate logic
does more than inspect the chosen plugins: it looks for a possible provider in
available fields, checks whether that provider fits, recursively checks the
provider's requirements, and detects cycles. It also distinguishes answered
single-plugin fields from extensible lists and considers nested axes.

An alternative UI could offer the choice and then explain what is missing, or
derive required calculators and ask the operator to configure ambiguous ones.
That could remove predictive search. It is not the same behavior as hiding
choices that cannot become valid in the current context.

### Example: changing the format

The form infers which later answers still equal their recommendations by cloning
the draft, substituting offered answers, filling their defaults, and comparing.
It then follows changes, prunes unwanted recommendations, and retains calculators
needed by choices the operator kept. `keep_what_is_needed` is effectively another
piece of dependency resolution in the form layer.

A simple "the user touched this field" flag is not equivalent. Manually choosing
the offered value still follows later changes, and a form loaded from a config
must behave the same without any interaction history. A later edit must also
leave earlier fields alone, even if their recommendations would now differ.

Explicitly applying recommendations, or separating requested calculators from
required derived calculators, would make the policy more declarative. But each
changes behavior. The latter must still support multiple providers, configurable
calculators, optional extra metrics, and the current persistence contract.

If every current interaction remains required, these policies need to be
preserved or replaced by equivalent algorithms. Moving them into one resolver
does not by itself establish a large net simplification.

## Other directions examined

| Direction | Assessment |
| --- | --- |
| Retire CLI and browser terminal | Removes a known 2,461-main-line boundary including the terminal test driver, but removes interfaces. Not acceptable under the current feature-preservation constraint. |
| Replace open plugins with closed enums | Could reduce framework work, but violates third-party openness. Also requires replacement configuration UI; the full framework size is not a savings estimate. |
| Recompute metrics after each result | Could shorten update code, but changes result-reporting costs. No recommendation without a representative benchmark, and the metric family already shares its main calculation. |
| Replace the async questionnaire with a manual state machine | Risks replacing compiler-generated suspension state with handwritten stacks, retry state, and list cursors. Not an obvious KISS improvement. |
| Keep only an async prompter trait | A bounded cleanup of duplicated sync/async signatures and forwarding. Worth considering, but not the substantial change being sought. |
| Merge every GUI/CLI view | Some policies differ, including metric column order and text layout. Sharing those indiscriminately can add options and adapters instead of deleting work. |
| Reclassify E2E helpers as tests | Changes the counters; does not simplify the source. The three E2E support crates currently contribute 2,107 main-category lines. |

## Recommendation

The authoritative field model has now been prototyped and rejected for adoption
in its current form; see the experiment below. It removed JSON editing helpers,
but did not establish a substantial simplification across the consumers.

If simpler configuration behavior is acceptable, evaluate it as a separate design
with explicit before/after examples. Removing predictive assistance may be a more
direct simplification than building a more elaborate generic solver to preserve it.

Do not start another production refactor based only on a file's size. The next
experiment should demonstrate deletion of a named mechanism and report its net
cost, including the replacement model, adapters, and caller changes.

## Experiment: authoritative fields and stable identities

Implemented on `experiments/authoritative-draft-fields`, commit `7dca743`.
The production branch retains the previous implementation. This is a completed
prototype evaluation, not a claim that every possible tree design is unsuitable.

The prototype made the draft own typed answers and nested plugin fields.
`Draft::fields` borrowed that tree rather than reconstructing a snapshot. Defaults
and compatibility queries traversed it. Stable identities addressed fields and
list items, and GUI callbacks submitted those identities through the app form.
Moving a list item retained its identity and those of its descendants; removing
or replacing a plugin invalidated its old descendants. JSON configs became a
cached export, invalidated on edits, rather than the authoritative editable data.
Existing path-based entry points remained as adapters.

The old `locate`, `set_in`, `edit`, and mutable JSON navigation were removed.
The separate compatibility `Question` representation was removed too. This was
more than adding a new tree in front of the old editing implementation.

| Rust code, excluding blanks and comments | Before | Prototype | Difference |
| --- | ---: | ---: | ---: |
| Main | 19,913 | 20,223 | +310 |
| Tests | 16,054 | 16,099 | +45 |
| Examples | 86 | 86 | 0 |

Line count alone does not decide this. Borrowed reads were easier to follow, and
the tree made the distinction between unanswered and explicit none visible in
types. But a maintainer now had to understand identity allocation, identity
lookup, stale identities, path maintenance after moves, typed answer conversion,
and invalidation of the serialization cache. The app still needed its form view
and rejected-input state. Recommendation following and predictive compatibility
were essentially unchanged, including their path-based scoping. The CLI still
used its existing path-based questionnaire API. Serialization and schema checking
also remained necessary for actual plugin construction and custom serde rules.

There is further work one could do, particularly sharing schema validation with
typed-answer parsing and changing the remaining consumers. Those omissions limit
the experiment: it does not prove a fully migrated design would be larger. But
the vertical slice did not justify migrating more code or introducing another
framework to manage identities. This version failed the stopping rule above.

All standard `prek run --all-files` hooks passed on the prototype, including
native tests, native and wasm Clippy, docs, formatting, and manifest checks.
Added tests exercised identity-preserving list moves, editing a moved descendant,
stale identities after removal, cache refresh, and transactional duplicate errors.
Browser E2E and wasm runtime tests were not run; the prototype was not adopted.

The finding narrows the next investigation: direct traversal is workable, but
replacing addresses does not remove the expensive-to-understand policies around
edits. A substantial reduction is more likely to require simpler recommendation
or dependency-selection behavior. That needs concrete operator-facing examples
and agreement on the behavior before implementation.
