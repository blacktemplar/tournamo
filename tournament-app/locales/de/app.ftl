# What tournament-app says in German: why an action was refused. A variable described as English in
# `en/app.ftl` is English here too.

error-empty-name = der Name eines Teilnehmers darf nicht leer sein
error-duplicate-name = es gibt schon einen Teilnehmer namens „{ $name }“
error-name-too-long = ein Name hat höchstens { $max } Zeichen
error-line-refused = Zeile { $number }, „{ $line }“: { $why }
error-entrant-added-after-start = das Turnier hat begonnen, daher kann kein Teilnehmer mehr hinzugefügt werden
error-unknown-entrant = diesen Teilnehmer gibt es nicht
error-entrant-removed-after-start = das Turnier hat begonnen, daher kann kein Teilnehmer mehr entfernt werden
error-no-rating = „{ $rating }“ ist keine Wertungszahl: eine Wertungszahl ist eine Zahl, etwa 1500
error-rating-after-start = das Turnier hat begonnen, daher können die Wertungszahlen nicht mehr geändert werden
error-unknown-format = es gibt kein Format „{ $format }“
error-format-misconfigured = das Format ist fehlerhaft: { $reason }
error-unknown-template = es gibt keine Vorlage „{ $template }“
error-template-does-not-expand = die Vorlage funktioniert nicht: { $reason }
error-invalid-template-phase = { $phase } kann so nicht gespielt werden; ihre Einstellungen zeigen, was zu ändern ist
error-setting-does-not-fit = { $phase } kann so nicht gespielt werden; ändere sie in den Einstellungen für Fortgeschrittene
error-has-phases = der Wettbewerb hat schon Phasen; eine Vorlage richtet einen Wettbewerb ohne Phasen ein
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
        [one] { $phases } nimmt Teilnehmer aus dieser Phase; entferne zuerst { $phases }
       *[other] { $phases } nehmen Teilnehmer aus dieser Phase; entferne zuerst jene Phasen
    }
error-later-phase-started = { $phase } hat mit den Teilnehmern dieser Phase begonnen; mache zuerst ihren Beginn rückgängig
error-phase-not-started = die Phase hat nicht begonnen
error-unstart-played = { $count ->
        [one] { $phases } hat Ergebnisse; öffne zuerst ihre Spiele wieder
       *[other] { $phases } haben Ergebnisse; öffne zuerst ihre Spiele wieder
    }
error-unstart-in-use = { $count ->
        [one] { $phases } hat mit Teilnehmern dieser Phase begonnen; mache zuerst ihren Beginn rückgängig
       *[other] { $phases } haben mit Teilnehmern dieser Phase begonnen; mache zuerst ihren Beginn rückgängig
    }
warning-cut-through-tie = in { $phase } teilen sich mehr Teilnehmer Platz { $rank }, als weiterkommen: das Los nimmt { $favoured }, nicht { $others }
warning-cut-through-tie-in-group = in Gruppe { $group } von { $phase } teilen sich mehr Teilnehmer Platz { $rank }, als weiterkommen: das Los nimmt { $favoured }, nicht { $others }
phase-unnamed = { $number } · { $format }
phase-progress = { $phase }: { $played } von { $matches } Spielen gespielt
error-no-phase = das Turnier hat noch keine Phase
error-result-does-not-fit = so geändert passt die Phase nicht zu { $count ->
        [one] dem Ergebnis
       *[other] den Ergebnissen
    } { $matches }; zuerst { $count ->
        [one] muss dieses Spiel
       *[other] müssen diese Spiele
    } wieder geöffnet werden
matches-of-edited-phase = { $count ->
        [one] ihres Spiels { $numbers }
       *[other] ihrer Spiele { $numbers }
    }
matches-of-later-phase = { $count ->
        [one] von Spiel { $numbers }
       *[other] der Spiele { $numbers }
    } in { $phase }, wohin sie Teilnehmer schickt
error-phase-change-does-not-fit = so kann die Phase nicht geändert werden, da eine spätere Änderung nicht mehr dazu passt: { $reason }
error-started-already = das Turnier hat schon begonnen
error-too-few-entrants = ein Turnier braucht mindestens zwei Teilnehmer
error-too-few-for-format = das Format braucht mindestens { $count } Teilnehmer
error-too-few-for-divisions = { $divisions } Gruppen brauchen mindestens { $count } Teilnehmer: verringere die Zahl der Gruppen
error-field-does-not-fit = { $phase } kann aus { $count } Teilnehmern keine Teams bilden ({ $teams }), weil die Zahl nicht aufgeht
warning-many-rounds = { $count } Runden sind mehr, als ein Turnier gewöhnlich spielt, und ein so großes wird langsam gespeichert: Ist die Phase so gemeint?
warning-too-many-groups = { $divisions } Gruppen brauchen mindestens { $count } Teilnehmer; { $teams } Teilnehmer ergeben höchstens { $most ->
        [one] eine Gruppe
       *[other] { $most } Gruppen
    }
warning-teams-unfit = aus { $count } Teilnehmern lassen sich keine Teams bilden ({ $teams }), weil die Zahl nicht aufgeht
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
error-name-elsewhere = „{ $name }“ ist in Spiel { $number } nur Teil eines Namens, während ein Teilnehmer dieses Namens in Spiel { $elsewhere } spielt: mehr vom Namen eingeben oder die Nummer des anderen Spiels
error-name-not-in-match = kein offenes Spiel { $number } hat einen Teilnehmer, dessen Name „{ $name }“ enthält
error-name-in-several-matches = „{ $name }“ passt zu { $matches } offenen Spielen; gib die Nummer von einem dazu, z. B. „{ $name } { $number }“
error-name-on-both-sides = „{ $name }“ passt zu beiden Seiten von Spiel { $number }; gib mehr vom Namen ein
error-scores-too-many = { $parts ->
        [0] Spiel { $number } nimmt keinen Spielstand: gib das Ergebnis ein, 1, X oder 2
        [one] Spiel { $number } nimmt nur einen Spielstand
       *[other] Spiel { $number } nimmt höchstens { $parts } Spielstände, einen pro Satz
    }
error-scores-not-whole = Spiel { $number } nimmt nur ganze Zahlen als Spielstand
error-match-not-playable-yet = { $count ->
        [0] Spiel { $number } kann noch nicht gespielt werden
        [one] Spiel { $number } kann noch nicht gespielt werden: es wartet auf Spiel { $waiting }
       *[other] Spiel { $number } kann noch nicht gespielt werden: es wartet auf die Spiele { $waiting }
    }
error-score-too-large = { $value } ist zu groß für einen Spielstand von Spiel { $number }: höchstens { $max }
error-not-a-score = „{ $word }“ ist kein Spielstand von Spiel { $number }: ein Spielstand sind zwei Zahlen, z. B. 21-17
error-not-a-result = „{ $word }“ ist kein Ergebnis von Spiel { $number }: gib 1, X oder 2 ein oder den Spielstand, z. B. 21-17
error-scores-undecided = die eingegebenen Spielstände sagen nicht, wer Spiel { $number } gewonnen hat; gib das Ergebnis dazu: 1, X oder 2
error-result-against-scores = das für Spiel { $number } eingegebene Ergebnis ist nicht das, das seine Spielstände sagen
error-match-not-reported = das Spiel hat kein Ergebnis, das zurückgenommen werden könnte
error-result-depended-on = ein späteres Ergebnis hängt von diesem ab; nimm zuerst jenes zurück
error-result-depended-on-by = { $count ->
        [one] das Ergebnis von Spiel { $numbers } hängt von diesem ab; zuerst muss Spiel { $numbers } wieder geöffnet werden
       *[other] die Ergebnisse der Spiele { $numbers } hängen von diesem ab; zuerst müssen die Spiele { $numbers } wieder geöffnet werden
    }
error-match-not-reported-to-edit = das Spiel hat kein Ergebnis, das geändert werden könnte
error-edit-depended-on = so geändert passt ein späteres Ergebnis nicht mehr; zuerst muss jenes Spiel wieder geöffnet werden
error-edit-does-not-fit = { $count ->
        [one] so geändert passt das Ergebnis von Spiel { $numbers } nicht mehr; zuerst muss Spiel { $numbers } wieder geöffnet werden
       *[other] so geändert passen die Ergebnisse der Spiele { $numbers } nicht mehr; zuerst müssen die Spiele { $numbers } wieder geöffnet werden
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
followed-shown = an { $cause } angepasst: { $settings }
followed-hidden = { $levels ->
    [expert] die Experteneinstellungen wurden
    [both] die Einstellungen für Fortgeschrittene und Experten wurden
   *[advanced] die Einstellungen für Fortgeschrittene wurden
} an { $cause } angepasst
followed-format = das Format { $plugin }
followed-match-format = das Spielformat { $plugin }
followed-change = die Änderung
field-label = { $field }
warning-unranked = ohne Kriterium sind in der Tabelle alle Teilnehmer gleichauf
warning-lot-decides-cut = die Teilnehmer beiderseits des Schnitts nach Platz { $place } der Anmeldung haben gleiche Wertungszahlen oder keine, daher entscheidet das Los, wer auf welcher Seite ist
places-in-groups = bei { $groups } Gruppen nebeneinander { $count ->
        [one] ist Platz { $from }
       *[other] sind die Plätze { $from }–{ $to }
    } { $parts }{ $compared ->
        [yes] , die eines Platzes im Vergleich der Gruppen
       *[no] {""}
    }
places-on-in-groups = bei { $groups } Gruppen nebeneinander sind die Plätze ab { $from } { $parts }{ $compared ->
        [yes] , die eines Platzes im Vergleich der Gruppen
       *[no] {""}
    }
group-place = Platz { $rank } jeder Gruppe
group-places = die Plätze { $first }–{ $last } jeder Gruppe
group-places-on = die Plätze ab { $rank } jeder Gruppe
group-places-best = { $count ->
        [one] der Beste
       *[other] die besten { $count }
    } auf Platz { $rank } ihrer Gruppe
group-places-worst = { $count ->
        [one] der Schlechteste
       *[other] die schlechtesten { $count }
    } auf Platz { $rank } ihrer Gruppe
group-place-between = Platz { $place } unter denen auf Platz { $rank } ihrer Gruppe
group-places-between = die Plätze { $first }–{ $last } unter denen auf Platz { $rank } ihrer Gruppe
list = { $first } und { $last }
warning-earlier-round = { $name } hat noch Spiel { $earlier } aus einer früheren Runde zu spielen
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
issue-unprovided-for = { $needer } braucht { $plugin }, wofür eines davon nötig ist: { $providers }
issue-unprovided-lacking = { $plugin } braucht { $count ->
        [one] { $providers }
       *[other] eines davon: { $providers }
    }, und dafür ist { $lacking } nötig, was nichts Gewähltes liefert
issue-unmet = { $plugin } braucht { $condition }, aber { $provider } ergibt { $provided }
issue-unmet-unsaid = { $plugin } braucht { $condition }, was { $provider } nicht ergibt
issue-unmet-coming-in = { $plugin } braucht { $condition }, aber es kommen { $provided } herein
issue-unmet-context = { $plugin } braucht { $condition }, was nicht hereinkommt
issue-provider = { $plugin } ({ $field })
issue-fitting = { $issue }; bei { $field } { $count ->
        [one] passt
       *[other] passen
    } { $choices }
