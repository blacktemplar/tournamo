# The plugins of tournament-engine in German, see `PluginText` in `src/i18n.rs` for the keys.

## The fields of a phase itself, which are no plugin's

phase =
    .field-format = Format
    .field-format-description = wie die Spiele angesetzt werden
    .field-match_format = Spielformat
    .field-match_format-description = was zu einem Spiel eingetragen wird
    .field-ranking = Reihenfolge der Tabelle
    .field-ranking-description = die Besten zuerst; jedes Kriterium entscheidet nur, wo die vor ihm gleich sind
    .field-metrics = Kennzahlen
    .field-metrics-description = für jeden Teilnehmer berechnet

## Match formats

plugin-tournament_engine-winner_only =
    .label = nur Sieger
    .description = wer ein Spiel gewonnen hat oder dass es unentschieden ausging, ohne Spielstand
    .level-match = Spiel
plugin-tournament_engine-scored =
    .label = Spielstand
    .description = die Punkte oder Tore jeder Seite
    .level-match = Spiel
    .value-score = Punkte
    .field-winning_score = Gewinnpunktzahl
    .field-winning_score-description = was ein Sieger üblicherweise hat, oder mehr, dann mit einem oder zwei Punkten Vorsprung; jeder andere Spielstand wird angemerkt, da er ein Tippfehler sein kann
plugin-tournament_engine-best_of =
    .label = Gewinnsätze
    .description = wer die meisten Sätze gewinnt, gewinnt; jeder Satz nach seinem Spielstand
    .level-match = Spiel
    .level-game = Satz
    .value-score = Punkte
    .field-games = Sätze
    .field-winning_score = Gewinnpunktzahl eines Satzes
    .field-winning_score-description = was der Sieger eines Satzes üblicherweise hat, oder mehr, dann mit einem oder zwei Punkten Vorsprung; jeder andere Spielstand wird angemerkt, da er ein Tippfehler sein kann

## Phase formats

plugin-tournament_engine-round_robin =
    .label = Jeder gegen jeden
    .description = alle spielen gegen alle
plugin-tournament_engine-random_rounds =
    .label = zufällige Runden
    .description = für viele Teilnehmer und wenig Zeit: einige Runden von Jeder gegen jeden, zufällig gewählt, sodass sich niemand zweimal trifft
    .field-rounds = Runden
    .field-rounds-description = ist Jeder gegen jeden kürzer, wird das ganz gespielt
plugin-tournament_engine-single_elimination =
    .label = K.-o.-System
    .description = wer ein Spiel verliert, scheidet aus; Unentschieden sind nicht erlaubt
    .field-third_place_match = Spiel um Platz drei
    .field-third_place_match-description = die beiden Verlierer der Halbfinale spielen es neben dem Finale; ohne es sind beide gemeinsam Dritte
    .stage-final = Finale
    .stage-semifinals = Halbfinale
    .stage-quarterfinals = Viertelfinale
    .stage-fraction-finals = { $fraction ->
        [8] Achtelfinale
        [16] Sechzehntelfinale
        [32] Zweiunddreißigstelfinale
        [64] Vierundsechzigstelfinale
       *[other] { $fraction }tel-Finale
    }
    .stage-third-place = Spiel um Platz drei
plugin-tournament_engine-double_elimination =
    .label = Doppel-K.-o.-System
    .description = wer zum zweiten Mal verliert, scheidet aus; Unentschieden sind nicht erlaubt
    .field-grand_final_reset = Wiederholung des großen Finales
    .field-grand_final_reset-description = verliert der Sieger der Gewinnerrunde das große Finale, spielen die beiden es noch einmal; ohne sie scheidet aus, wer das große Finale verliert
    .stage-winners-round = Gewinnerrunde { $round }
    .stage-winners-final = Finale der Gewinnerrunde
    .stage-losers-round = Verliererrunde { $round }
    .stage-losers-final = Finale der Verliererrunde
    .stage-grand-final = Großes Finale
    .stage-grand-final-reset = Wiederholung des großen Finales
plugin-tournament_engine-monster_dyp =
    .label = Monster-DYP
    .description = „Draw Your Partner“ wie beim Tischfußball: jede Runde werden die Teilnehmer zu Zweierteams gelost, die gegeneinander spielen, und jeder Teilnehmer wird für sich gewertet
    .field-rounds = Runden
    .field-rounds-description = alle werden zu Beginn gelost

## Metrics calculators

plugin-tournament_engine-points =
    .label = Punkte
    .description = für jeden Sieg, jedes Unentschieden und jedes Freilos
    .column = Punkte
    .field-points_for_win = Punkte für einen Sieg
    .field-points_for_draw = Punkte für ein Unentschieden
    .field-points_for_bye = Punkte für ein Freilos
    .field-points_for_bye-description = eine Runde ohne Spiel, z. B. bei einer ungeraden Zahl an Teilnehmern
plugin-tournament_engine-wins_draws_losses =
    .label = Bilanz aus Siegen, Unentschieden und Niederlagen
    .description = wie viele Spiele gewonnen, unentschieden gespielt und verloren wurden
    .column = S-U-N
    .cell-wins-draws-losses = { $wins }-{ $draws }-{ $losses }
plugin-tournament_engine-points_balance =
    .label = erzielte und erhaltene Punkte
    .description = die Punkte oder Tore aller Spiele, wie die Ergebnisse sie angeben
    .column = +:−
    .cell-scored-conceded = { $scored }:{ $conceded }
plugin-tournament_engine-games_balance =
    .label = gewonnene und verlorene Sätze
    .description = die Sätze aller Spiele, wie die Ergebnisse sie angeben
    .column = Sätze
    .cell-won-lost = { $won }:{ $lost }
plugin-tournament_engine-buchholz =
    .label = Buchholz
    .description = die Punkte aller Gegner
    .column = Bh
plugin-tournament_engine-buchholz_of_buchholz =
    .label = Fein-Buchholz
    .description = die Buchholz-Wertung aller Gegner
    .column = FBh
plugin-tournament_engine-sonneborn_berger =
    .label = Sonneborn-Berger
    .description = die Punkte der besiegten Gegner und derer, gegen die unentschieden gespielt wurde
    .column = SB
    .field-weight_for_win = Gewicht eines besiegten Gegners
    .field-weight_for_win-description = die Punkte dieses Gegners werden damit multipliziert
    .field-weight_for_draw = Gewicht eines Gegners, gegen den unentschieden gespielt wurde
    .field-weight_for_draw-description = die Punkte dieses Gegners werden damit multipliziert
plugin-tournament_engine-round_of_first_loss =
    .label = Runde der ersten Niederlage
    .description = die Runde, in der ein Teilnehmer zum ersten Mal verloren hat
    .column = verloren in
    .cell-round = Runde { $round }
    .cell-unbeaten = -
plugin-tournament_engine-round_of_elimination =
    .label = Runde des Ausscheidens
    .description = die Runde, in der ein Teilnehmer ausgeschieden ist
    .column = raus in
    .cell-round = Runde { $round }
    .cell-still-in = -

## Ranking criteria

plugin-tournament_engine-higher_points_difference =
    .label = höhere Punktedifferenz
    .description = die erzielten Punkte oder Tore abzüglich der erhaltenen
plugin-tournament_engine-more_points_scored =
    .label = mehr erzielte Punkte
    .description = eine Feinwertung, die Angreifen über Verteidigen stellt
plugin-tournament_engine-higher_games_difference =
    .label = höhere Satzdifferenz
    .description = die gewonnenen Sätze abzüglich der verlorenen
plugin-tournament_engine-more_games_won =
    .label = mehr gewonnene Sätze
    .description = eine Feinwertung, die die gewonnenen Sätze zählt, nicht die verlorenen

plugin-tournament_engine-more_points =
    .label = mehr Punkte
    .description = meist das erste Kriterium; die Punkte legen fest, was jedes Ergebnis wert ist
plugin-tournament_engine-more_wins =
    .label = mehr Siege
    .description = eine Feinwertung, die Siege über Unentschieden stellt
plugin-tournament_engine-more_draws =
    .label = mehr Unentschieden
    .description = eine Feinwertung, die Unentschieden über Siege wie über Niederlagen stellt
plugin-tournament_engine-fewer_losses =
    .label = weniger Niederlagen
    .description = eine Feinwertung, die Unentschieden über Niederlagen stellt
plugin-tournament_engine-fewer_matches_played =
    .label = weniger gespielte Spiele
    .description = dasselbe Ergebnis aus weniger Spielen ist besser
plugin-tournament_engine-more_buchholz =
    .label = höhere Buchholz-Wertung
    .description = eine Feinwertung zugunsten derer, die auf stärkere Gegner trafen
plugin-tournament_engine-more_buchholz_of_buchholz =
    .label = höhere Fein-Buchholz-Wertung
    .description = eine Feinwertung, falls auch die Buchholz-Wertung gleich ist, nach den Gegnern der Gegner
plugin-tournament_engine-more_sonneborn_berger =
    .label = höhere Sonneborn-Berger-Wertung
    .description = eine Feinwertung zugunsten derer, die stärkere Gegner besiegten oder gegen sie unentschieden spielten
plugin-tournament_engine-later_first_loss =
    .label = spätere erste Niederlage
    .description = nach der Runde der ersten Niederlage; nie verloren zu haben ist am besten
plugin-tournament_engine-later_elimination =
    .label = späteres Ausscheiden
    .description = nach der Runde des Ausscheidens; noch dabei zu sein ist am besten
