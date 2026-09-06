#!/usr/bin/env python3
"""Estrae il campione di finding da sottoporre al giudizio umano.

Il campionamento è **stratificato per tipo di finding e per corpus**, non
proporzionale alla frequenza: se lo fosse, la famiglia più numerosa occuperebbe
quasi tutto il campione e le altre non verrebbero giudicate affatto. Serve
sapere *quali* famiglie sono utili, non ripetere trenta volte la stessa.

Il file prodotto non contiene nessuna statistica aggregata: chi etichetta deve
farlo prima di vedere i numeri, o li starebbe confermando invece che misurando.
"""

import json
import random
import sys

random.seed(20260830)


def famiglia(finding: dict) -> str:
    """Il tipo di finding, dedotto dalla prima riga di evidenza."""
    motivo = finding["evidence"][0] if finding["evidence"] else ""
    if finding["classification"] == "Shadow":
        return "shadow"
    if finding["classification"] == "Known":
        return "known"
    if finding["classification"] == "Zombie":
        return "zombie"
    if "never observed" in motivo:
        return "dichiarato-mai-visto"
    if "no exact match" in motivo:
        return "match-parziale"
    if "not declared" in motivo:
        return "metodo-non-dichiarato"
    return "altro"


def main() -> None:
    quanti = 30
    per_corpus = {}
    for percorso in sys.argv[1:]:
        etichetta = percorso.split("/")[-1].replace("-report.json", "")
        documento = json.load(open(percorso))
        gruppi = {}
        for finding in documento["findings"]:
            gruppi.setdefault(famiglia(finding), []).append(finding)
        per_corpus[etichetta] = gruppi

    # Quote uguali per famiglia, distribuite fra i corpus che la contengono.
    famiglie = sorted({f for gruppi in per_corpus.values() for f in gruppi})
    quota = max(1, quanti // (len(famiglie) or 1))
    campione = []
    for f in famiglie:
        for corpus, gruppi in sorted(per_corpus.items()):
            disponibili = gruppi.get(f, [])
            if not disponibili:
                continue
            for finding in random.sample(disponibili, min(quota, len(disponibili))):
                campione.append((corpus, finding))
    random.shuffle(campione)
    campione = campione[:quanti]

    print("# Trenta finding da giudicare\n")
    print("Per ciascuno: **utile** (vale la pena guardarlo), **rumore** (non lo")
    print("guarderei), **non so**. Scrivi il giudizio nella riga `giudizio:`.\n")
    print("Il campione è stratificato per tipo e per applicazione, e **non** è")
    print("proporzionale a quanto ciascun tipo compare davvero: serve a capire quali")
    print("famiglie valgono, non a misurarne il volume. Nessuna statistica in questo")
    print("file, di proposito.\n")
    print("---\n")
    for numero, (corpus, finding) in enumerate(campione, 1):
        print(f"## {numero}. {finding['classification']} — {corpus}\n")
        print(f"- endpoint: `{finding['path_pattern']}`")
        print(f"- confidenza: {finding['confidence']} · severità: {finding['severity']}")
        for riga in finding["evidence"]:
            print(f"- perché: {riga}")
        print("\n  giudizio: \n")


if __name__ == "__main__":
    main()
