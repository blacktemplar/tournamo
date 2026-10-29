# GUI review (2026-10-29)

Five reviewers each drove the release build (`dx build -p tournament-gui --release`) in headless Chrome and judged only what a user sees, without reading the code. Each had one focus:

- the first-time setup,
- running a tournament live,
- look and feel across viewports and themes,
- accessibility, keyboard use and German,
- robustness and edge cases.

The viewports were 320 to 1920 px wide (phone with touch emulation, tablet, laptop, wide), in light and dark, in English and German. No data was lost, and nothing was logged to the console in any session.

Swiss and multi-phase events were requested but do not exist yet, so they are left out of the findings.

Severity is **H**igh, **M**edium, **L**ow or **N**it. "×n" means n reviewers found it independently. The screenshots were taken to `/tmp/gui-review/` and are not kept.

## Findings

### A. Silent failures and data correctness

| ID | Sev | Finding |
|----|-----|---------|
| A1 | H ×3 | **An invalid number is silently dropped on submit.** Type `-1` in "Points for a win": the issue only shows once the field loses focus, often below the visible part of the edit dialog. Add phase or Save changes is still accepted and keeps the last valid value. The input has no `aria-invalid`, and the message is not announced. |
| A2 | H | **A double-tap on a phone wipes a result.** Once a result is in, the card shrinks and "Reopen" appears exactly where "Draw"/"Wins" was. A second tap 250 ms later reopens the match ("Match 2 is open again"). |
| A3 | H | **The first click after choosing a Format is swallowed** (new-phase form only; the edit dialog is fine). Pick "Single elimination", then tick "Match for third place": it stays unticked, and the phase is added without it. Reproduced three times, on desktop and on the phone. |
| A4 | H | **Escape in a dropdown inside the edit-phase dialog closes the whole dialog and discards the edits.** No warning is shown. |
| A5 | L | **A format change looks as if it wiped the results** (reported as "drops every result"; not reproduced). Results are never dropped. A change keeps those whose match the new format still schedules, and it is refused ("match does not exist") if a reported match no longer fits. What remains: after the change the match list jumps to round 2, where every match is open, so at a glance the results seem gone. The refusal is worded in engine terms and doesn't say which result is in the way. |
| A6 | M | **An imported file with `"name": ""` becomes an invisible, unclickable entry in the list.** Its buttons are named "Rename ⁨⁩". Creating and renaming refuse a blank name; importing does not. |
| A7 | M | **Monster DYP with 3 entrants starts and finishes at once** ("Finished, tied for first: A, B, C", three gold medals). Start should be refused. |
| A8 | L | **Rounds = 1000000 freezes the page for about 14 s.** The round list then holds 16960 rounds (1,000,000 mod 65,536), so something seems to be **16-bit**. `99999999999999` is refused as "not a whole number of at least 1". |
| A9 | L | **A double-click shows a false error.** After the first click works, the second says "The match has a result already" or "The competition has started already". |
| A10 | L | **Error banners stay after later successful actions.** Two alerts can then show at once (an old import error plus "A competition needs a name"). |
| A11 | M ×3 | **Undo is gone after a reload, after leaving the competition, or after another tab takes over.** Nothing explains it, and "Reopen" is the only way back. |

### B. Responsive layout (mostly phone)

| ID | Sev | Finding |
|----|-----|---------|
| B1 | H ×3 | **Names break mid-word in narrow match cards.** Examples: "Zimmerma/nn", "Hohenzoller/n-/Sigmaringe/n", and "Di/et/er" at 320 px. In German, "Gewinnt" is clipped to "Gewinn" at 400 px. This breaks "names never cut". |
| B2 | H ×2 | **On a phone the standings scroll sideways.** Points and the tie-breaks are off-screen, W-D-L is clipped ("4-1-("), and names wrap while Rank and Played waste width. |
| B3 | H ×3 | **Gaps and overlaps around the sticky tabs.** Content shows through the gap between the top bar and the tabs. The cross table's column header and the bracket's round headings slide under the tabs, and an element scrolled into view ends up under them (a click meant for "Draw" hit the tabs). |
| B4 | M | **The page scrolls sideways at 320 px**, in both languages: the view switch is 324 px wide and "Phase" is cut off. |
| B5 | M ×3 | **The storage warning takes up most of the screen.** It fills 40–45 % of the first phone screen, sits above the entrants on every competition and on the list, and its buttons are 36 px under a coarse pointer. |
| B6 | M ×2 | **The announcement line causes empty space and jumps.** It reserves about 50 px (about 100 px on a phone) even when empty, and is plain grey text once filled. The Finished banner pushes the match list down by about 110 px. |
| B7 | M ×2 | **The bracket is too tall and gives no scroll hint.** For 20 entrants, 12 bye boxes make it about 1,400 px tall, so it can't be projected. On a phone only round 1 is visible and nothing hints at more rounds to the right. |
| B8 | M | **Checkboxes are 13 px with 22 px labels under a coarse pointer**, while the other controls are 44 px. |
| B9 | L | **Undo, Redo and Export lose their labels too early,** already at 820 px. The title is cut off on phones ("Friday Night D…"). |
| B10 | L | **The header shifts about 15 px between tabs** when the scrollbar disappears. Fix: `scrollbar-gutter: stable`. |
| B11 | L | **The bar's content doesn't line up with the page at 1920** (title at x=20, content at x=248). |
| B12 | L | **The setup screen is boxy on a phone.** Boxes sit inside boxes (card, then section, then indented group). The sort buttons wrap below their item, and the add-entrant field drifts down as the list grows. |
| B13 | N | **The "Cross table" tab wraps onto two lines on a phone,** making the tabs uneven. |

### C. Visual design

| ID | Sev | Finding |
|----|-----|---------|
| C1 | M ×2 | **Clickable names in the bracket look like web links** (underlined, blue) and add noise. A single tap reports a result without confirmation, which is risky while showing the bracket to players. |
| C2 | M | **Winner and loser look almost the same.** Both are bold and only a small ✓ marks the winner. Played cards get a dashed border, which usually means "placeholder". |
| C3 | M | **In the dark theme, cards barely stand out from the page background.** |
| C4 | L | **The standings are noisy before any match is played.** Every row shows "1 · 0 · – · 0-0-0", and in a knockout the entrants with byes rank above the winners. |
| C5 | L ×2 | **The cross table lacks what readers expect.** There is no total column and no W/D/L tint, and the bold names outweigh the results. It is sparse with 24 entrants in random rounds. |
| C6 | L | **Page tabs and the in-card "Playable now / By round" switch look identical.** Custom selects (Format) and native ones (round picker, footer) are mixed. |
| C7 | N | **Some controls carry too much weight, and the home page has no logo.** The 24 outlined "Remove" buttons are heavy, and the three icon buttons of a competition row look equally important (delete should be quieter). |
| C8 | M | **The highlighted option in a dropdown is barely visible:** about 1.1:1 contrast (WCAG 2.4.7/1.4.11). |

### D. Accessibility and focus

| ID | Sev | Finding |
|----|-----|---------|
| D1 | H ×3 | **Focus falls to `<body>` after Create, Start and removing an entrant.** After Start the page also keeps its old scroll position and nothing is announced. |
| D2 | H | **Escape in a dropdown outside a dialog leaves the focus on `<body>`** instead of its trigger. |
| D3 | M | **Reporting in the bracket moves focus to the next match's winner button.** One more Enter reports the final. |
| D4 | M ×2 | **The "Draw" buttons have no context:** a screen reader hears "Draw" twelve times, while the win buttons say "Alice wins". |
| D5 | M | **The typed-result preview is only referenced by `aria-describedby`**, so it isn't announced while typing. |
| D6 | M ×2 | **The delete confirmation opens with focus on Delete,** so a double Enter deletes. It should focus Cancel and be an `alertdialog`. |
| D7 | L | **Some changes are not announced:** adding or removing an entrant, adding a phase, saving a phase. The old announcement stays visible and in the status region. |
| D8 | L | **Switching the language re-renders a stale error inside `role=alert`**, so it is announced again. |
| D9 | L ×3 | **Errors are shown in a banner at the top instead of at their field:** an empty competition name, a duplicate entrant. The input gets no `aria-invalid`/`aria-describedby`, and the banner pushes the page down. |
| D10 | L | **The edit dialog opens with focus on "About format"**, whose toggletip covers the format's description. |
| D11 | L ×2 | **Some fields and buttons have no visible label.** The entrant and competition name fields have only a placeholder. Icon-only buttons (rename, export, delete; undo, redo and export when narrow) have no visible label or tooltip. |
| D12 | L | **Tables and the switch lack names.** The standings have no caption, and their name cells are `<td>`, not `<th scope=row>`. The "Playable now / By round" switch has no group label. |
| D13 | L | **The final announcement doesn't name the winner** ("Every match has a result."). |
| D14 | N | **The bracket hint says "Click or tap",** leaving out keyboard users. |
| D15 | L | **Toggletips stay open while their button has focus** and cover the next row. The reviewers disagree on whether Escape closes them; check. |

### E. Wording, onboarding, guidance

| ID | Sev | Finding |
|----|-----|---------|
| E1 | H | **The help covers only storage, offline use and open source.** It says nothing about the workflow (entrants → phase → start → results), what each format does, the tie-breaks (Buchholz, Sonneborn-Berger), byes or undo. It is reachable only from a small link in the footer. |
| E2 | H | **The default round robin starts in chess terms:** "More Sonneborn-Berger", the Buchholz metric, "Weight of an opponent played to a draw", draws worth 0.5. The toggletips are terse lowercase fragments that never say when to choose what, and the ones on criteria describe the metric ("Fewer losses" explains W-D-L). Nothing explains how "Metrics" relate to "Order of the standings". |
| E3 | M | **Two format names say little to newcomers:** "Random rounds" (what it is good for) and "Monster DYP" (foosball jargon, though its description is clear). |
| E4 | M | **Nothing shows the scale before Start.** Round robin is "(recommended)" even for 24 entrants, which means 23 rounds and 276 matches, and this only becomes visible after starting. |
| E5 | M | **The edit-phase dialog's intro is engine-speak:** "The phase is played as if it had been added this way. The results reported stay as long as it still schedules their matches." |
| E6 | L | **A locked metric doesn't name what needs it:** "Buchholz, since another one needs it" (it is "More Buchholz" in the order). |
| E7 | L | **Fields that don't apply are still shown, and one lacks a toggletip.** "Points for a draw" is shown for eliminations, which allow no draws. "Rounds" (Monster DYP) has no toggletip. |
| E8 | L | **The metric checkboxes change order when another format is chosen.** |
| E9 | N | **The landing page has no introduction** saying what Tournamo is or what the steps are. |
| E10 | N | **Some texts are fragments or inconsistent.** Hints and toggletips start in lowercase (worse in German: "die Nummer des Spiels…"). "winner's" has a straight apostrophe. "Lost in: round 1". |
| E11 | L | **Rounds are named three ways in a knockout:** the picker says "Round 1–5", the cards and bracket say "Round of 32" (with 20 players) and "Final", and the standings say "Lost in: round 5" for the runner-up. "Round 1" and "Round of 32" sit side by side with no separator. |
| E12 | L | **The hint under "Type in results" is always visible**, even after the competition has finished, and takes 6 lines on a phone. The input stays after the finish. |
| E13 | M | **Typing a name rarely picks a match in pre-scheduled formats.** In round robin "ali" fits 7 open matches, and in random rounds "anna" fits 3, so the number is always needed. |
| E14 | L | **German wording:** "Sieg: Eve" should be "Sieger: Eve"; "Mehr Sonneborn-Berger" should be "Höhere Sonneborn-Berger-Wertung"; "…wo er zu mehreren passt" should be "…falls er auf mehrere passt, die Nummer des Spiels"; "Vorige Runde" could be "Vorherige Runde". "Teilnehmer hinzufügen" squeezes the name field to about 155 px. |
| E15 | L | **Every import failure gives the same message,** "The file is no competition this version can open." This covers empty, garbage, a newer format and a damaged log. The details show raw parser text ("EOF while parsing…", "reads formats 1 to 1 at line 1 column 12", "TransitionId(…) already appears"). |
| E16 | N | **The delete confirmation says "Export it first" but offers no Export button.** |
| E17 | L ×3 | **Duplicate entrants:** a banner and an inline hint both appear for an exact duplicate. A case variant ("alice") is only warned about, and Enter adds it. |

### F. Entrants and competition management

| ID | Sev | Finding |
|----|-----|---------|
| F1 | M | **Entrants can't be renamed** (fixing a typo means remove and re-add, which changes the order), **and a pasted list doesn't add several at once.** |
| F2 | L | **Renaming a competition accepts an existing name,** while import adds "(n)". |
| F3 | N | **Navigation details.** The selected tab is not in the URL, so reload and back/forward forget it, and a new competition opens on the tab last used elsewhere. `#nonexistent` shows the list silently, while an unknown UUID says "There is no such competition". |
| F4 | N | **Removing an entrant needs no confirmation.** That is acceptable, since undo works. |

### What works well (keep it)

- **Entering data is fast.** Name and Enter keeps the focus in the field: 24 or even 64 entrants go in without the mouse. One tap reports a result. Typed results ("3 1", "2x", "p19") show a live preview and precise errors.
- **Undo, redo and Reopen are reliable** (98 quick steps stayed consistent). Their announcements are clear ("Undone: the result of match 28 (Bob won)").
- **Multi-tab:** the read-only tab is clearly labelled, catches up within about 1 s and takes over when the other tab closes. Deleting a competition open elsewhere is refused with a reason.
- **Reload and offline keep everything.** Export and import round-trip, and duplicates get "(n)".
- **Names are robust:** emoji, Arabic, HTML-like text and 200-character names display safely and wrap. Blank and duplicate names are refused, and whitespace is trimmed.
- **Structure is sound:** good landmarks and headings, a visible focus ring, a logical tab order, and proper dialogs (modal, labelled, focus returns). Button names are meaningful and `aria-disabled` is used correctly. The bracket and cross table have hidden texts and headers for screen readers.
- **The look is calm and neutral, and the dark theme is complete.** Error boxes are clear, and the two-column layout works from 1180 px.

## Plan

The steps are ordered by impact relative to effort: cheap fixes for silent failures first, large features last.

- **Effort** is a rough guess made without reading the code: **S** is about an hour, **M** is about a day, **L** is several days or needs a design decision first.
- Logic belongs in `tournament-app` with tests; each step is a commit, or a few.
- New layout behaviour needs an e2e test in `tournament-gui-e2e`, where it can be expressed.

| # | Step | Fixes | Impact | Effort |
|---|------|-------|--------|--------|
| 1 | **Refuse a phase form with issues.** `PhaseForm` reports invalid answers, so Add phase and Save changes refuse them and focus the first invalid field. Set `aria-invalid` and link the message with `aria-describedby`. Show the issue as you type, not only on leaving the field. | A1, part of D9 | H | S–M |
| 2 | **Fix the dropdowns.** Escape closes only the listbox (stop it reaching the dialog) and returns the focus to the trigger. Find why the click after a selection is swallowed (probably the closing listbox or its overlay taking the next pointer event). Give the highlighted option a token background or outline with at least 3:1 contrast. | A3, A4, D2, C8 | H | S–M |
| 3 | **Keep "Reopen" out of reach of a double-tap.** Put it at a different spot on the card than the result buttons, or make it ignore input for about 600 ms after it appears. | A2 | H | S |
| 4 | **Fix the sticky tabs.** Put them at `top: var(--bar-height)` on an opaque background. Make the cross table's and bracket's headers stick below them, and set `scroll-padding-top` to bar plus tabs. Add `scrollbar-gutter: stable`. Let the view switch wrap or scroll within itself at 320 px. | B3, B10, B4, B13 | H | S |
| 5 | **Fix focus and announcements.** After Create, focus the entrant field. After Start, move to the Matches heading and announce "Started: N matches". After removing an entrant, focus the next Remove button or the field. After a bracket report, focus the bracket heading. The delete dialog focuses Cancel as an `alertdialog`. Announce entrant, phase and save actions, and name the winner at the end. Open the edit dialog on its heading or Format field, not the toggletip. | D1, D3, D6, D7, D10, D13 | H | S–M |
| 6 | **Name things for screen readers.** "Draw" becomes "Alice – Bob: draw" (`aria-label`). The typed-result preview gets `role="status"`. The standings get a caption and row headers. The view switch gets a group label. Icon-only buttons get a `title`. The bracket hint stops saying "Click or tap". | D4, D5, D11, D12, D14 | M | S |
| 7 | **Clear stale errors and ignore repeats.** Clear the page's error at the next action and on a language change. Ignore an action while the same one is still running, so a double-click is not reported as "already". | A9, A10, D8 | M | S |
| 8 | **Validate edge cases.** Import gives a blank name a fallback. A format declares its minimum entrant count (Monster DYP: 4) and Start is refused below it. Rounds gets a sensible `max`. Look into the 16-bit wraparound. | A6, A7, A8 | M | S |
| 9 | **Redo the match cards on phones.** Below a breakpoint, stack the two sides, each name on a full-width line with its button. Allow breaks only at spaces and hyphens (`overflow-wrap: normal`). Mute the loser, give played cards a solid quiet border, and keep the dashed border for placeholders. | B1, C2 | H | M |
| 10 | **Fix the standings on phones.** Keep Rank and Name in place and put the first ranking column right after Name. Let the tie-breaks scroll with an edge fade. Mute metric cells before the first result. | B2, C4 | H | M |
| 11 | **Make the storage warning and announcements compact.** Collapse the warning to one line with "More" and 44 px buttons, placed below the tabs. Keep the live region visually hidden and show announcements as a toast (later with an Undo button). Place the Finished banner so it doesn't push the matches down. | B5, B6 | M | M |
| 12 | **Make a format change understandable.** After the change, keep the round view where it was (or show the first round with results) instead of jumping to the first open round. Word the refusal for the operator: which reported result the new format doesn't schedule, and that undoing it allows the change. Rewrite the dialog's intro in plain words: results stay, and a change that doesn't fit them is refused. | A5, E5 | L–M | S–M |
| 13 | **Do a wording pass.** Write toggletips and hints as sentences, and give each criterion its own toggletip. Name what a locked metric is needed by. Name rounds consistently in knockouts. Add the German fixes from E14. Distinguish import failures (not a Tournamo file / newer version / damaged) and fix "formats 1 to 1". Add an Export button to the delete dialog. Keep only one message for a duplicate entrant. | E6, E10, E11, E14, E15, E16, E17, D9 | M | M |
| 14 | **Polish the visuals.** Bracket names get a button look (row highlight, no underline). Dark-theme cards get a lighter surface token. Larger custom checkboxes with 44 px rows. Page tabs become underlined, distinct from the in-card switch, and native selects are styled to match. Lower the breakpoint for icon-only header buttons to about 600 px. Align the bar's content with the page at wide widths. Flatten the nested boxes on the setup screen. Make Remove and delete quieter. | C1, C3, C6, C7, B8, B9, B11, B12 | M | M |
| 15 | **Make the type-in work better.** If a name fits exactly one match that can be played now (or one in the earliest open round), pick it. Fold the hint behind its ⓘ after the first success, and hide the input once finished. | E13, E12 | M | M |
| 16 | **Set expectations for undo.** In the short term, a tooltip on a disabled Undo ("Undo covers changes made since opening this competition"). Persisting the session history across a reload is a design decision: DESIGN.md keeps it unpersisted on purpose. | A11 | M | S (note) / L (persist) |
| 17 | **Improve entrant management.** Rename entrants inline (only the name changes, kept in `People`, not the log). Pasting several lines adds several entrants. Refuse case-insensitive duplicates, or ask first. Warn about a duplicate competition name. | F1, F2, E17 | M | M |
| 18 | **Make the bracket compact.** Draw an entrant with a bye in its next round with a "bye" tag instead of a full box. Add edge fades or arrows when the bracket scrolls, and keep round headings sticky. Later, a fit-to-screen view for projecting. | B7 | M | L |
| 19 | **Onboard newcomers.** Add a help section on running a competition, with a glossary of formats and tie-breaks, linked from the toggletips and the empty setup. Offer scoring presets ("3/1/0", "Chess") and fold the metrics into an "Advanced" section. Show the expected rounds and matches next to the format, and base the recommendation on the number of entrants. Give "Random rounds" and "Monster DYP" descriptions by purpose. Hide fields that don't apply. Keep the metric order stable. Add a one-line introduction on the landing page. | E1, E2, E3, E4, E7, E8, E9 | H | L |
| 20 | **Smaller additions.** The cross table gets a total column and subtle W/D/L tints. The selected tab goes into the URL. `#unknown` says there is no such competition. | C5, F3 | L | M |

Toggletip behaviour (D15) should be checked during step 2 or 13, since the reviewers' reports contradict each other.
