# The plugins and metrics of tournament-engine in English: nothing but where a screen says it
# otherwise, since the English of each is what its code says. A message here replaces it, see
# `PluginText` and `MetricText` in `src/i18n.rs` for the keys.

## The fields of a phase itself, which are no plugin's

# the code's "ranked by" is what the CLI asks, followed by "then" for each further criterion; as
# the heading of a list, it read as half a sentence
phase =
    .field-ranking = order of the standings

## Metrics

# the headers of the standings, abbreviated as chess does, so that the columns of numbers stay
# narrow; the toggletip of the header says the whole name
metric-tournament_engine-buchholz =
    .column = Bh
metric-tournament_engine-buchholz_of_buchholz =
    .column = BhBh
metric-tournament_engine-sonneborn_berger =
    .column = SB
