# The plugins and metrics of tournament-engine in German, see `PluginText` and `MetricText` in
# `src/i18n.rs` for the keys.

## What plugins require of each other, said where they do not fit together

facts =
    .points = die Punkte
    .buchholz = die Buchholz-Wertung
    .games-won = die gewonnenen Sätze
    .games-balance = die gewonnenen und verlorenen Sätze
    .wins-draws-losses = die Siege, Unentschieden und Niederlagen
    .points-scored = die erzielten Punkte
    .points-balance = die erzielten und erhaltenen Punkte
    .eliminations = ein Format, das sagt, welche Spiele ausscheiden lassen
    .ranked-metric = die Wertung, nach der es ordnet
    .entrants-per-qualifier = die hereinkommenden Teilnehmer
    .single-entrants-coming-in = einzeln hereinkommende Teilnehmer
    .entrants-per-ranking = die gewerteten Teilnehmer
    .single-players = einzelne Spieler
    .single-entrants = einzelne Teilnehmer
    .teams-of = Teams aus { $count }

## The fields of a phase itself, which are no plugin's

phase =
    .field-format = Format
    .field-format-description = wie die Spiele angesetzt werden
    .field-teams = Teams
    .field-teams-description = ob die Teilnehmer allein oder zusammen spielen
    .field-divisions = Gruppen
    .field-divisions-description = in wie vielen Teilen die Phase gespielt wird, jeder für sich
    .field-draw = Auslosung
    .field-draw-description = wie die Teilnehmer auf die Gruppen verteilt werden
    .field-seeding = Setzung
    .field-seeding-description = in welcher Reihenfolge das Format die Teilnehmer einer Gruppe setzt
    .field-match_format = Spielformat
    .field-match_format-description = was zu einem Spiel eingetragen wird
    .field-ranking = Reihenfolge der Tabelle
    .field-ranking-description = die Besten zuerst; jedes Kriterium entscheidet nur, wo die vor ihm gleich sind
    .field-metrics = Kennzahlen
    .field-metrics-description = für jeden Teilnehmer berechnet
    .field-across = gruppenübergreifend verglichen nach
    .field-across-description = nach dem Platz in der Gruppe: wie Teilnehmer desselben Platzes in verschiedenen Gruppen gegeneinander gereiht werden

## Match formats

plugin-tournament_engine-winner_only =
    .label = nur Sieger
    .description = wer gewonnen hat oder, wo das Format Unentschieden zulässt, ob es unentschieden ausging, ohne Spielstand
    .level-match = Spiel
plugin-tournament_engine-scored =
    .label = Spielstand
    .description = die Punkte oder Tore jeder Seite
    .level-match = Spiel
    .value-score = Punkte
    .field-winning_score = Gewinnpunktzahl
    .field-winning_score-description = was ein Sieger üblicherweise hat, oder mehr, dann mit einem oder zwei Punkten Vorsprung; jeder andere Spielstand wird angemerkt, da er ein Tippfehler sein kann
plugin-tournament_engine-best_of =
    .label = nach Sätzen
    .description = wer die meisten Sätze gewinnt, gewinnt; jeder Satz nach seinem Spielstand
    .level-match = Spiel
    .level-game = Satz
    .value-score = Punkte
    .field-games = Best of (Sätze)
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
    .field-grand_final_reset-description = verliert der Sieger der Gewinnerrunde das große Finale, spielen die beiden es noch einmal; ohne Wiederholung scheidet aus, wer das große Finale verliert
    .stage-winners-final = Finale der Gewinnerrunde
    .stage-winners-semifinals = Halbfinale der Gewinnerrunde
    .stage-winners-quarterfinals = Viertelfinale der Gewinnerrunde
    .stage-winners-fraction-finals = { $fraction ->
        [8] Achtelfinale der Gewinnerrunde
        [16] Sechzehntelfinale der Gewinnerrunde
        [32] Zweiunddreißigstelfinale der Gewinnerrunde
        [64] Vierundsechzigstelfinale der Gewinnerrunde
       *[other] { $fraction }tel-Finale der Gewinnerrunde
    }
    .stage-losers-round = Verliererrunde { $round }
    .stage-losers-final = Finale der Verliererrunde
    .stage-grand-final = Großes Finale
    .stage-grand-final-reset = Wiederholung des großen Finales
plugin-tournament_engine-monster_dyp =
    .label = Monster-DYP
    .description = „Draw Your Partner“ wie beim Tischfußball: jede Runde werden die Teilnehmer zu Zweierteams gelost, die gegeneinander spielen, und jeder Teilnehmer wird für sich gewertet
    .field-rounds = Runden
    .field-rounds-description = alle werden zu Beginn gelost

## Draws

plugin-tournament_engine-snake =
    .label = hin und zurück
    .description = die Gruppen bekommen die Teilnehmer abwechselnd, in ihrer Reihenfolge, hin und zurück
plugin-tournament_engine-random =
    .label = zufällig
    .description = jeder Teilnehmer in eine Gruppe nach Los
plugin-tournament_engine-blocks =
    .label = in Blöcken
    .description = die Besten in die erste Gruppe, die Nächsten in die zweite und so weiter; jede Gruppe spielt um eigene Plätze

## Seedings

plugin-tournament_engine-seeded_in_order =
    .label = nach Rang
    .description = die Teilnehmer werden nach ihren Plätzen in den Phasen davor gesetzt, sonst nach Wertungszahl; die ohne Wertungszahl nach Los
plugin-tournament_engine-seeded_at_random =
    .label = zufällig
    .description = die Teilnehmer werden nach Los gesetzt
plugin-tournament_engine-group_winners_apart =
    .label = Gruppen getrennt
    .description = für ein K.-o.-System der Besten mehrerer Gruppen: Teilnehmer derselben Gruppe treffen so spät wie möglich aufeinander

## Teams

plugin-tournament_engine-keep =
    .label = unverändert
    .description = alle spielen, wie sie kommen: aus der Anmeldung allein, aus einer Phase mit Teams in ihrem Team
plugin-tournament_engine-random_pairs =
    .label = geloste Paare
    .description = die Teilnehmer werden zu Paaren gelost, die durchgehend zusammen spielen
plugin-tournament_engine-best_with_worst =
    .label = Beste mit Schlechtesten
    .description = paart den Ersten mit dem Letzten, den Zweiten mit dem Vorletzten und so weiter: aus der Anmeldung in der Reihenfolge der Wertungszahlen, ausgelost, wo es keine gibt, aus einer Phase nach ihren Plätzen
plugin-tournament_engine-split =
    .label = alle allein
    .description = die Teams einer Phase mit Teams werden aufgeteilt, sodass ihre Mitglieder allein spielen; für die Anmeldung dasselbe wie unverändert

## Metrics

metric-tournament_engine-points =
    .label = Punkte
    .description = für jeden Sieg, jedes Unentschieden und jedes Freilos
    .column = Punkte
metric-tournament_engine-points_per_match =
    .label = Punkte pro Spiel
    .description = die in Spielen erzielten Punkte geteilt durch die gespielten Spiele
    .column = P/S
metric-tournament_engine-wins_draws_losses =
    .label = Bilanz aus Siegen, Unentschieden und Niederlagen
    .description = wie viele Spiele gewonnen, unentschieden gespielt und verloren wurden
    .column = S-U-N
    .cell-wins-draws-losses = { $wins }-{ $draws }-{ $losses }
metric-tournament_engine-points_balance =
    .label = erzielte und erhaltene Punkte
    .description = die Punkte oder Tore aller Spiele, wie die Ergebnisse sie angeben
    .column = +:−
    .cell-scored-conceded = { $scored }:{ $conceded }
metric-tournament_engine-points_difference =
    .label = Punktedifferenz
    .description = die erzielten Punkte oder Tore abzüglich der erhaltenen
    .column = ±
metric-tournament_engine-games_balance =
    .label = gewonnene und verlorene Sätze
    .description = die Sätze aller Spiele, wie die Ergebnisse sie angeben
    .column = Sätze
    .cell-won-lost = { $won }:{ $lost }
metric-tournament_engine-games_difference =
    .label = Satzdifferenz
    .description = die gewonnenen Sätze abzüglich der verlorenen
    .column = Sätze ±
metric-tournament_engine-buchholz =
    .label = Buchholz
    .description = die Punkte aller Gegner
    .column = Bh
metric-tournament_engine-buchholz_of_buchholz =
    .label = Fein-Buchholz
    .description = die Buchholz-Wertung aller Gegner
    .column = FBh
metric-tournament_engine-sonneborn_berger =
    .label = Sonneborn-Berger
    .description = die Punkte der besiegten Gegner und derer, gegen die unentschieden gespielt wurde
    .column = SB
metric-tournament_engine-round_of_first_loss =
    .label = Runde der ersten Niederlage
    .description = die Runde, in der ein Teilnehmer zum ersten Mal verloren hat
    .column = verloren in
    .cell-unbeaten = -
metric-tournament_engine-round_of_elimination =
    .label = Runde des Ausscheidens
    .description = die Runde, in der ein Teilnehmer ausgeschieden ist
    .column = raus in
    .cell-still-in = -

## Metrics calculators

plugin-tournament_engine-points =
    .label = Punkte
    .description = für jeden Sieg, jedes Unentschieden und jedes Freilos
    .field-points_for_win = Punkte für einen Sieg
    .field-points_for_draw = Punkte für ein Unentschieden
    .field-points_for_bye = Punkte für ein Freilos
    .field-points_for_bye-description = eine Runde ohne Spiel, z. B. bei einer ungeraden Zahl an Teilnehmern
plugin-tournament_engine-points_per_match =
    .label = Punkte pro Spiel
    .description = die in Spielen erzielten Punkte geteilt durch die gespielten Spiele
plugin-tournament_engine-wins_draws_losses =
    .label = Bilanz aus Siegen, Unentschieden und Niederlagen
    .description = wie viele Spiele gewonnen, unentschieden gespielt und verloren wurden
plugin-tournament_engine-points_balance =
    .label = erzielte und erhaltene Punkte
    .description = die Punkte oder Tore aller Spiele, wie die Ergebnisse sie angeben
plugin-tournament_engine-points_difference =
    .label = Punktedifferenz
    .description = die erzielten Punkte oder Tore abzüglich der erhaltenen
plugin-tournament_engine-games_balance =
    .label = gewonnene und verlorene Sätze
    .description = die Sätze aller Spiele, wie die Ergebnisse sie angeben
plugin-tournament_engine-games_difference =
    .label = Satzdifferenz
    .description = die gewonnenen Sätze abzüglich der verlorenen
plugin-tournament_engine-buchholz =
    .label = Buchholz
    .description = die Punkte aller Gegner
plugin-tournament_engine-buchholz_of_buchholz =
    .label = Fein-Buchholz
    .description = die Buchholz-Wertung aller Gegner
plugin-tournament_engine-sonneborn_berger =
    .label = Sonneborn-Berger
    .description = die Punkte der besiegten Gegner und derer, gegen die unentschieden gespielt wurde
    .field-weight_for_win = Gewicht eines besiegten Gegners
    .field-weight_for_win-description = die Punkte dieses Gegners werden damit multipliziert
    .field-weight_for_draw = Gewicht eines Gegners, gegen den unentschieden gespielt wurde
    .field-weight_for_draw-description = die Punkte dieses Gegners werden damit multipliziert
plugin-tournament_engine-round_of_first_loss =
    .label = Runde der ersten Niederlage
    .description = die Runde, in der ein Teilnehmer zum ersten Mal verloren hat
plugin-tournament_engine-round_of_elimination =
    .label = Runde des Ausscheidens
    .description = die Runde, in der ein Teilnehmer ausgeschieden ist

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
plugin-tournament_engine-more_points_per_match =
    .label = mehr Punkte pro Spiel
    .description = vergleicht Teilnehmer mit verschieden vielen Spielen, z. B. in verschieden großen Gruppen
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

## Presets

plugin-tournament_engine-groups_then_knockout =
    .label = Gruppen mit anschließendem K.-o.-System
    .description = alle spielen gegen alle anderen ihrer Gruppe, dann spielen die Besten jeder Gruppe im K.-o.-System weiter
    .field-groups = Gruppen
    .field-advancing = Aufsteiger je Gruppe
    .phase-group-phase = Gruppenphase
    .phase-knockout = K.-o.-Phase
plugin-tournament_engine-qualification_then_knockouts =
    .label = Qualifikation mit anschließenden K.-o.-Turnieren
    .description = einige Runden gegen zufällige Gegner ordnen alle, die dann nach ihren Plätzen in K.-o.-Turnieren spielen: die Besten im ersten, die Nächsten im zweiten und so weiter
    .field-rounds = Runden
    .field-knockouts = K.-o.-Turniere
    .phase-qualification = Qualifikation
    .phase-knockouts = K.-o.-Turniere
plugin-tournament_engine-dyp =
    .label = DYP
    .description = Draw Your Partner: die Teilnehmer werden zu Paaren gelost, die im Doppel-K.-o.-System spielen
    .phase-dyp = DYP
