# What tournament-app says in German: why an action was refused. A variable described as English in
# `en/app.ftl` is English here too.

error-empty-name = der Name eines Teilnehmers darf nicht leer sein
error-duplicate-name = es gibt schon einen Teilnehmer namens „{ $name }“
error-entrant-added-after-start = das Turnier hat begonnen, daher kann kein Teilnehmer mehr hinzugefügt werden
error-unknown-entrant = diesen Teilnehmer gibt es nicht
error-entrant-removed-after-start = das Turnier hat begonnen, daher kann kein Teilnehmer mehr entfernt werden
error-no-rating = „{ $rating }“ ist keine Wertungszahl: eine Wertungszahl ist eine Zahl, etwa 1500
error-rating-after-start = das Turnier hat begonnen, daher können die Wertungszahlen nicht mehr geändert werden
error-unknown-format = es gibt kein Format „{ $format }“
error-format-misconfigured = das Format ist fehlerhaft: { $reason }
error-invalid-phase = so kann die Phase nicht gespielt werden; das Formular zeigt, was zu ändern ist
error-no-sources = wähle, woher die Phase ihre Teilnehmer nimmt
error-source-empty = ein Bereich von Plätzen endet, bevor er beginnt
error-sources-overlap = zwei Bereiche von Plätzen der Phase überschneiden sich
error-source-taken = { $phase } nimmt manche dieser Plätze schon
error-registration-closed = das Turnier hat begonnen, daher kann eine Phase nicht mehr allein aus der Anmeldung nehmen
error-mixed-team-sizes = die genommenen Plätze sind von Teams verschiedener Größe, etwa Einzelnen und Paaren, die eine Phase nur mit „alle allein“ nimmt
error-place-not-taken = gib einen Platz ein: eine ganze Zahl ab 1
error-no-such-phase = diese Phase gibt es nicht
error-phase-started-already = die Phase hat schon begonnen
error-phase-starts-with-competition = die Phase nimmt ihre Teilnehmer aus der Anmeldung und beginnt mit dem Turnier
error-phase-waiting = die Phase kann beginnen, sobald ihre Teilnehmer feststehen: { $phases }
error-phase-removed-after-start = die Phase hat begonnen, daher kann sie nicht mehr entfernt werden
error-phase-in-use = { $count ->
        [one] { $phases } nimmt Teilnehmer aus dieser Phase; entferne zuerst diese
       *[other] { $phases } nehmen Teilnehmer aus dieser Phase; entferne zuerst diese
    }
error-later-phase-started = { $phase } hat mit den Teilnehmern dieser Phase begonnen; mache zuerst ihren Beginn rückgängig
warning-cut-through-tie = { $favoured } und { $others } teilen sich Platz { $rank } in { $phase }; das Los reiht { $favoured } vor
warning-cut-through-tie-in-group = { $favoured } und { $others } teilen sich Platz { $rank } der Gruppe { $group } in { $phase }; das Los reiht { $favoured } vor
phase-unnamed = { $number } · { $format }
phase-progress = { $phase }: { $played } von { $matches } Spielen gespielt
error-no-phase = das Turnier hat noch keine Phase
error-result-does-not-fit = { $count ->
        [one] so geändert spielt die Phase Spiel { $numbers } nicht, das schon ein Ergebnis hat; um sie so zu ändern, muss zuerst Spiel { $numbers } wieder geöffnet werden
       *[other] so geändert spielt die Phase die Spiele { $numbers } nicht, die schon Ergebnisse haben; um sie so zu ändern, müssen zuerst die Spiele { $numbers } wieder geöffnet werden
    }
error-phase-change-does-not-fit = so kann die Phase nicht geändert werden, da eine spätere Änderung nicht mehr dazu passt: { $reason }
error-started-already = das Turnier hat schon begonnen
error-too-few-entrants = ein Turnier braucht mindestens zwei Teilnehmer
error-too-few-for-format = das Format braucht mindestens { $count } Teilnehmer
error-too-few-for-divisions = { $divisions } Gruppen brauchen mindestens { $count } Teilnehmer: verringere die Zahl der Gruppen
error-field-does-not-fit = { $phase } kann aus { $count } Teilnehmern keine Teams bilden ({ $teams }), etwa aus einer ungeraden Zahl keine Paare
error-not-started = das Turnier hat noch nicht begonnen
error-unknown-match = dieses Spiel gibt es nicht
error-match-finished-already = das Spiel hat schon ein Ergebnis
error-draw-not-allowed = ein Spiel dieses Formats kann nicht unentschieden enden
error-undecided = das Ergebnis sagt nicht, wer gewonnen hat
error-value-not-taken = ein eingegebener Wert wird nicht angenommen; korrigiere den markierten
error-result-unreadable = gib die Nummer eines Spiels und sein Ergebnis ein, 1, X oder 2, z. B. „3 1“, oder einen Teil des Namens des Siegers
error-no-match-numbered = es gibt kein Spiel { $number }
error-match-numbered-finished-already = Spiel { $number } hat schon ein Ergebnis
error-result-missing = gib auch das Ergebnis von Spiel { $number } ein: 1, X oder 2
error-no-name-found = kein offenes Spiel hat einen Teilnehmer, dessen Name „{ $entered }“ enthält
error-name-not-in-match = kein offenes Spiel { $number } hat einen Teilnehmer, dessen Name „{ $name }“ enthält
error-name-in-several-matches = „{ $name }“ passt zu { $matches } offenen Spielen; gib die Nummer von einem dazu, z. B. „{ $name } { $number }“
error-name-on-both-sides = „{ $name }“ passt zu beiden Seiten von Spiel { $number }; gib mehr vom Namen ein
error-scores-not-taken = Spiel { $number } nimmt die eingegebenen Spielstände nicht
error-scores-undecided = die eingegebenen Spielstände sagen nicht, wer Spiel { $number } gewonnen hat; gib das Ergebnis dazu: 1, X oder 2
error-result-against-scores = das für Spiel { $number } eingegebene Ergebnis ist nicht das, das seine Spielstände sagen
error-match-not-reported = das Spiel hat kein Ergebnis, das zurückgenommen werden könnte
error-result-depended-on = ein späteres Ergebnis hängt von diesem ab; nimm zuerst jenes zurück
error-result-depended-on-by = { $count ->
        [one] das Ergebnis von Spiel { $numbers } hängt von diesem ab; zuerst muss Spiel { $numbers } wieder geöffnet werden
       *[other] die Ergebnisse der Spiele { $numbers } hängen von diesem ab; zuerst müssen die Spiele { $numbers } wieder geöffnet werden
    }
error-nothing-to-undo = es gibt nichts rückgängig zu machen
error-nothing-to-redo = es gibt nichts wiederherzustellen
error-undo-changed-elsewhere = eine inzwischen anderswo gemachte Änderung verhindert, dass dies rückgängig gemacht wird, daher kann nichts mehr rückgängig gemacht werden
error-redo-changed-elsewhere = eine inzwischen anderswo gemachte Änderung verhindert, dass dies wiederhergestellt wird, daher kann nichts mehr wiederhergestellt werden
error-competition-not-found = dieses Turnier gibt es nicht
error-competition-unreadable = das Turnier kann nicht gelesen werden
error-competition-newer = das Turnier wurde von einer neueren Version von Tournamo gespeichert, die diese nicht öffnen kann; mit einem Neuladen der Seite kommt die neueste
error-competition-damaged = das Turnier ist beschädigt gespeichert
error-log-does-not-replay = das Protokoll lässt sich nicht nachspielen
error-file-no-competition = die Datei ist kein Turnier von Tournamo
error-file-newer = die Datei stammt von einer neueren Version von Tournamo, die diese nicht öffnen kann; mit einem Neuladen der Seite kommt die neueste
error-file-damaged = die Datei ist ein Turnier von Tournamo, aber beschädigt
error-rejected = abgelehnt: { $reason }

cell-round = Runde { $round }

## The form of a phase

number-range = { $whole ->
        [yes] eine ganze Zahl
       *[no] eine Zahl
    }{ $bounds ->
        [both] {" "}von { $min } bis { $max }
        [min] {" "}ab { $min }
        [max] {" "}bis { $max }
       *[none] {""}
    }{ $multiple ->
        [0] {""}
       *[other] , die ein Vielfaches von { $multiple } ist
    }
warning-unranked = ohne Kriterium sind in der Tabelle alle Teilnehmer gleichauf
warning-lot-decides-cut = die Teilnehmer beiderseits des Schnitts nach Platz { $place } der Anmeldung haben gleiche Wertungszahlen oder keine, daher entscheidet das Los, wer auf welcher Seite ist
warning-below-winning-score = keine Seite hat { $winning }, die ein Sieger üblicherweise hat
warning-winning-lead = ab { $winning } führt ein Sieger üblicherweise um eins oder zwei
warning-result-disagrees = das ist nicht das Ergebnis, das der Spielstand nahelegt
warning-more-than-decided = es ist mehr eingetragen, als es zur Entscheidung des Spiels brauchte
warning-more-than-decided-at = das Spiel war bei { $a }–{ $b } schon entschieden, trotzdem ist mehr eingetragen
issue-unanswered = das braucht eine Antwort
issue-out-of-range = das muss { number-range } sein
issue-not-a-number = „{ $entered }“ ist keine Zahl
issue-duplicate = das ist doppelt gewählt
issue-rejected = das wird nicht angenommen: { $reason }
issue-invalid = { $reason }
issue-incompatible = das passt nicht zu den anderen: { $reason }
issue-unprovided = { $plugin } braucht { $fact }, was nichts liefert
issue-unprovided-by = { $plugin } braucht eines davon: { $providers }
