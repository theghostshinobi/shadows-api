#!/usr/bin/env python3
"""Guida l'applicazione sotto collaudo attraverso il reverse proxy.

Il valore di questo script non sta nel traffico che inventa, ma nel fatto che
**le forme degli URL le decide l'applicazione**, non io: nomi utente, nomi di
repository, indici di issue, SHA di commit e percorsi di file sono quelli che
Gitea usa davvero. È la differenza fra una fixture e un corpus.
"""

import json
import random
import sys
import urllib.error
import urllib.request

BASE = "http://127.0.0.1:8088"
TOKEN = sys.argv[1]
random.seed(20260830)  # deterministico: il corpus si rigenera identico

UTENTI = ["alessandra", "bruno", "chiara", "davide", "elisa", "fabio", "giulia", "luca"]
REPO = ["backend-api", "frontend", "infra-tooling", "docs-portal", "billing-service", "log-parser"]


def chiama(metodo, percorso, corpo=None, api=True):
    url = f"{BASE}{'/api/v1' if api else ''}{percorso}"
    dati = json.dumps(corpo).encode() if corpo is not None else None
    richiesta = urllib.request.Request(url, data=dati, method=metodo)
    richiesta.add_header("Authorization", f"token {TOKEN}")
    richiesta.add_header("User-Agent", "collaudo-shadow/1.0")
    if dati:
        richiesta.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(richiesta, timeout=30) as risposta:
            testo = risposta.read()
            return risposta.status, (json.loads(testo) if testo[:1] in (b"{", b"[") else None)
    except urllib.error.HTTPError as errore:
        return errore.code, None
    except Exception:
        return 0, None


def main():
    creati = {"utenti": [], "repo": [], "issue": [], "commit": []}

    for nome in UTENTI:
        stato, _ = chiama("POST", "/admin/users", {
            "username": nome, "email": f"{nome}@example.test",
            "password": "collaudo-Password-1", "must_change_password": False,
        })
        if stato in (201, 422):
            creati["utenti"].append(nome)

    for nome in REPO:
        stato, corpo = chiama("POST", "/user/repos", {
            "name": nome, "private": False, "auto_init": True,
            "description": f"repository di collaudo {nome}",
        })
        if stato in (201, 409):
            creati["repo"].append(nome)

    proprietario = "collaudo"
    for repo in creati["repo"]:
        for indice in range(1, 13):
            stato, corpo = chiama("POST", f"/repos/{proprietario}/{repo}/issues", {
                "title": f"problema numero {indice} su {repo}",
                "body": "aperto dal collaudo",
            })
            if stato == 201 and corpo:
                creati["issue"].append((repo, corpo["number"]))

        for nome_file in ["README.md", "docs/guida.md", "src/main.rs", "config/app.yaml"]:
            stato, corpo = chiama("POST", f"/repos/{proprietario}/{repo}/contents/{nome_file}", {
                "content": "Y29sbGF1ZG8K", "message": f"aggiunge {nome_file}",
            })
            if stato == 201 and corpo and corpo.get("commit"):
                creati["commit"].append((repo, corpo["commit"]["sha"]))

    # Lettura: è qui che nasce il grosso del traffico, come in un servizio vero.
    for _ in range(3):
        for repo in creati["repo"]:
            chiama("GET", f"/repos/{proprietario}/{repo}", api=True)
            chiama("GET", f"/repos/{proprietario}/{repo}/issues", api=True)
            chiama("GET", f"/repos/{proprietario}/{repo}/branches", api=True)
            chiama("GET", f"/{proprietario}/{repo}", api=False)
            chiama("GET", f"/{proprietario}/{repo}/issues", api=False)
            chiama("GET", f"/{proprietario}/{repo}/src/branch/main/README.md", api=False)
        for repo, numero in creati["issue"]:
            chiama("GET", f"/repos/{proprietario}/{repo}/issues/{numero}", api=True)
            chiama("GET", f"/{proprietario}/{repo}/issues/{numero}", api=False)
        for repo, sha in creati["commit"]:
            chiama("GET", f"/repos/{proprietario}/{repo}/git/commits/{sha}", api=True)
            chiama("GET", f"/{proprietario}/{repo}/commit/{sha}", api=False)
        for utente in creati["utenti"]:
            chiama("GET", f"/users/{utente}", api=True)
            chiama("GET", f"/users/{utente}/repos", api=True)
            chiama("GET", f"/{utente}", api=False)

    # Qualche richiesta che fallisce, come in un log vero.
    for percorso in ["/repos/collaudo/inesistente", "/users/nessuno", "/repos/collaudo/backend-api/issues/9999"]:
        chiama("GET", percorso)
    for percorso in ["/admin", "/user/settings", "/explore/repos", "/api/swagger"]:
        chiama("GET", percorso, api=False)

    print(json.dumps({k: len(v) for k, v in creati.items()}))


if __name__ == "__main__":
    main()
