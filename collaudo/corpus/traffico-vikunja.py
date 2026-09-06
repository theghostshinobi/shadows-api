#!/usr/bin/env python3
"""Guida Vikunja attraverso il proxy. Stessa logica del driver di Gitea: le
forme degli URL le decide l'applicazione, non lo script."""
import json, sys, urllib.error, urllib.request

BASE = "http://127.0.0.1:8089/api/v1"


def chiama(metodo, percorso, corpo=None, token=None):
    dati = json.dumps(corpo).encode() if corpo is not None else None
    r = urllib.request.Request(BASE + percorso, data=dati, method=metodo)
    r.add_header("User-Agent", "collaudo-shadow/1.0")
    if token:
        r.add_header("Authorization", f"Bearer {token}")
    if dati:
        r.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(r, timeout=30) as risposta:
            testo = risposta.read()
            return risposta.status, (json.loads(testo) if testo[:1] in (b"{", b"[") else None)
    except urllib.error.HTTPError as e:
        return e.code, None
    except Exception:
        return 0, None


def main():
    chiama("POST", "/register", {"username": "collaudo", "email": "collaudo@example.test",
                                 "password": "collaudo-Password-1"})
    stato, corpo = chiama("POST", "/login", {"username": "collaudo", "password": "collaudo-Password-1"})
    if not corpo or "token" not in corpo:
        print("login fallito", stato); return
    token = corpo["token"]

    progetti, task = [], []
    for nome in ["Piattaforma", "Fatturazione", "Documentazione", "Infrastruttura", "Supporto"]:
        stato, corpo = chiama("PUT", "/projects", {"title": nome}, token)
        if stato in (200, 201) and corpo:
            progetti.append(corpo["id"])

    for progetto in progetti:
        for i in range(1, 16):
            stato, corpo = chiama("PUT", f"/projects/{progetto}/tasks",
                                  {"title": f"attivita {i} del progetto {progetto}"}, token)
            if stato in (200, 201) and corpo:
                task.append(corpo["id"])

    for _ in range(3):
        chiama("GET", "/projects", None, token)
        chiama("GET", "/user", None, token)
        chiama("GET", "/tasks/all", None, token)
        for progetto in progetti:
            chiama("GET", f"/projects/{progetto}", None, token)
            chiama("GET", f"/projects/{progetto}/views", None, token)
            chiama("GET", f"/projects/{progetto}/projectusers", None, token)
        for identificativo in task:
            chiama("GET", f"/tasks/{identificativo}", None, token)
            chiama("GET", f"/tasks/{identificativo}/comments", None, token)

    for percorso in ["/projects/99999", "/tasks/99999", "/info"]:
        chiama("GET", percorso, None, token)

    print(json.dumps({"progetti": len(progetti), "task": len(task)}))


if __name__ == "__main__":
    main()
