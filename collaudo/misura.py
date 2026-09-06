#!/usr/bin/env python3
"""Misura le proprietà descritte in COLLAUDO.md a partire dai report di Shadow.

Non tocca il tool e non ne conosce l'interno: legge solo l'output pubblico
`shadow-report/1`. Se un giorno lo schema passasse a /2 senza che questo script
venga aggiornato, se ne accorge e lo dice invece di produrre numeri sbagliati.

    python3 collaudo/misura.py report.json [altri.json ...] [--log access.log]

Con --log esegue anche la controprova sulla sovra-aggregazione descritta in
COLLAUDO.md §3.
"""

import argparse
import collections
import json
import re
import sys

SCHEMA_ATTESO = "shadow-report/1"

# Le stesse forme tipo-ID del ruleset (shadow-core/src/ruleset.rs). Sono
# duplicate qui di proposito: questo script deve poter dire se il *tool* ha
# sbagliato, e per farlo non può usare il giudizio del tool.
LUNGHEZZE_HEX = {24, 32, 40, 64}
UUID = re.compile(r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$")
RIGA_NGINX = re.compile(r'^\S+ \S+ \S+ \[[^\]]+\] "(?:\S+) (\S+) [^"]*" ')


def tipo_id(segmento: str) -> bool:
    if segmento.isdigit() and segmento:
        return True
    if UUID.match(segmento):
        return True
    return len(segmento) in LUNGHEZZE_HEX and all(c in "0123456789abcdefABCDEF" for c in segmento)


def prefisso_padre(pattern: str) -> str:
    taglio = pattern.rfind("/")
    return "/" if taglio <= 0 else pattern[:taglio]


def carica(percorso: str) -> dict:
    with open(percorso) as f:
        documento = json.load(f)
    schema = documento.get("schema")
    if schema != SCHEMA_ATTESO:
        sys.exit(
            f"{percorso}: schema '{schema}', atteso '{SCHEMA_ATTESO}'. "
            "Lo schema è versionato apposta: aggiorna questo script prima di fidarti dei numeri."
        )
    return documento


def titolo(testo: str) -> None:
    print(f"\n=== {testo} ===")


def quota(parte: int, totale: int) -> str:
    return "n/d" if totale == 0 else f"{100 * parte / totale:.1f}%"


def misura_rumore(documento: dict) -> None:
    titolo("1. Rumore")
    finding = documento["findings"]
    endpoint = documento["endpoints"]
    per_categoria = collections.Counter(f["classification"] for f in finding)
    da_guardare = [f for f in finding if f["classification"] != "Known"]

    for categoria, quanti in sorted(per_categoria.items()):
        print(f"  {categoria:<14} {quanti}")
    print(f"  endpoint osservati            {len(endpoint)}")
    print(f"  silenziati dall'allowlist     {documento['findings_silenced_by_allowlist']}")
    if endpoint:
        print(f"  finding non-Known ogni 10 endpoint  {10 * len(da_guardare) / len(endpoint):.1f}")

    titolo("1b. Da quale motivo arriva il volume")
    motivi = collections.Counter(
        f["evidence"][0][:80] if f["evidence"] else "(senza evidenza)" for f in da_guardare
    )
    for motivo, quanti in motivi.most_common():
        print(f"  {quanti:>5}  {motivo}")


def misura_leggibilita(documento: dict) -> None:
    titolo("2. Leggibilità sotto redazione")
    endpoint = documento["endpoints"]
    pattern_redatti = sum(1 for e in endpoint if "<redacted>" in e["path_pattern"])
    righe = [riga for f in documento["findings"] for riga in f["evidence"]]
    righe_redatte = sum(1 for riga in righe if "<redacted>" in riga)

    print(f"  pattern con <redacted>        {pattern_redatti}/{len(endpoint)}  ({quota(pattern_redatti, len(endpoint))})")
    print(f"  evidenza con <redacted>       {righe_redatte}/{len(righe)}  ({quota(righe_redatte, len(righe))})")
    print("  soglia proposta: oltre il 15% il Redactor è troppo aggressivo")
    if not documento.get("redaction_enabled", True):
        print("  ATTENZIONE: questo report è stato prodotto con la redazione SPENTA")


def misura_normalizzazione(documento: dict, soglia_figli: int) -> None:
    titolo("3. Normalizzazione: candidati a forma di ID non riconosciuta")
    figli = collections.defaultdict(list)
    for e in documento["endpoints"]:
        pattern = e["path_pattern"]
        ultimo = pattern.rsplit("/", 1)[-1]
        if ultimo != "{id}":
            figli[prefisso_padre(pattern)].append((ultimo, e["observation_count"]))

    sospetti = sorted(
        ((p, f) for p, f in figli.items() if len(f) > soglia_figli),
        key=lambda voce: -len(voce[1]),
    )
    if not sospetti:
        print(f"  nessun prefisso con più di {soglia_figli} figli letterali")
    for prefisso, valori in sospetti:
        campione = ", ".join(v for v, _ in sorted(valori)[:5])
        print(f"  {len(valori):>5} figli letterali sotto {prefisso}")
        print(f"           campione: {campione}")
    print("  ogni riga è un candidato: guarda il campione e chiediti se è un identificatore")


def misura_parsing(documento: dict) -> None:
    titolo("4. Parsing")
    conteggi = documento["manifest"]["counts"]
    totale = conteggi["lines_total"]
    scartate = conteggi["lines_discarded"]
    print(f"  righe totali                  {totale}")
    print(f"  righe parsate                 {conteggi['lines_parsed']}")
    print(f"  righe scartate                {scartate}  ({quota(scartate, totale)})")
    for motivo, quanti in sorted(conteggi["discarded_by_reason"].items()):
        print(f"    {motivo:<22} {quanti}")
    print("  soglia proposta: oltre il 2% su un combined genuino la grammatica è troppo stretta")


def controprova_sovra_aggregazione(documento: dict, percorso_log: str) -> None:
    """Verifica che nessun segmento non-tipo-ID sia sparito dentro un `{id}`.

    È l'unica evidenza disponibile sul fallimento invisibile della Fase 2: la
    sovra-aggregazione non si vede dal report, perché ciò che è sparito non
    lascia traccia. Qui si guarda il log grezzo e si controlla che ogni parola
    ci sia ancora.
    """
    titolo("3b. Controprova: nessuna parola inghiottita dentro {id}")
    parole = collections.Counter()
    with open(percorso_log, "rb") as f:
        for riga in f:
            trovato = RIGA_NGINX.match(riga.decode("utf-8", "replace"))
            if not trovato:
                continue
            path = trovato.group(1).split("?", 1)[0]
            for segmento in path.split("/"):
                if segmento and not tipo_id(segmento):
                    parole[segmento] += 1

    presenti = set()
    for e in documento["endpoints"]:
        presenti.update(s for s in e["path_pattern"].split("/") if s)

    mancanti = [(parola, quante) for parola, quante in parole.most_common() if parola not in presenti]
    if not mancanti:
        print(f"  {len(parole)} segmenti non-tipo-ID nel log, tutti presenti nel report")
        return
    print(f"  {len(mancanti)} segmenti presenti nel log e assenti dal report:")
    for parola, quante in mancanti[:20]:
        print(f"    {quante:>6}x  {parola}")
    print("  ATTENZIONE: se uno di questi è un endpoint reale, è stato inghiottito.")
    print("  (una redazione può spiegarne alcuni: un segmento mascherato non combacia)")


def main() -> None:
    parser = argparse.ArgumentParser(description="Misura i report di Shadow per il collaudo.")
    parser.add_argument("report", nargs="+", help="report JSON prodotti da shadow --output-format json")
    parser.add_argument("--log", help="log grezzo, per la controprova sulla sovra-aggregazione")
    parser.add_argument("--soglia-figli", type=int, default=20,
                        help="oltre quanti figli letterali un prefisso è sospetto (default: 20)")
    argomenti = parser.parse_args()

    for percorso in argomenti.report:
        documento = carica(percorso)
        manifest = documento["manifest"]
        print(f"\n{'#' * 70}\n# {percorso}")
        print(f"# shadow {manifest['shadow_version']}, ruleset {manifest['ruleset_version']}")
        specifica = any(i["role"] == "openapispec" for i in manifest["inputs"])
        print(f"# inventario dichiarato: {'sì' if specifica else 'NO — solo euristiche'}")

        misura_rumore(documento)
        misura_leggibilita(documento)
        misura_normalizzazione(documento, argomenti.soglia_figli)
        misura_parsing(documento)
        if argomenti.log:
            controprova_sovra_aggregazione(documento, argomenti.log)

    print("\nQuello che questi numeri non dicono: se un finding sia utile.")
    print("Serve il passo umano di COLLAUDO.md — trenta finding etichettati a mano,")
    print("prima di guardare queste statistiche.")


if __name__ == "__main__":
    main()
