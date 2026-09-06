# Trenta finding da giudicare

Per ciascuno: **utile** (vale la pena guardarlo), **rumore** (non lo
guarderei), **non so**. Scrivi il giudizio nella riga `giudizio:`.

> ## Chi ha etichettato, e perché conta
>
> **Etichettato dall'agente il 2026-09-06, su autorizzazione esplicita del
> fondatore.** Va scritto qui e non solo nel resoconto, perché chi rileggerà
> questo file fra sei mesi deve sapere di chi è il giudizio che ci trova dentro.
>
> COLLAUDO.md aveva riservato questo passo al fondatore con una ragione precisa:
> *«io non so quali endpoint del tuo dominio meritino attenzione»*. Su questo
> corpus l'obiezione è più debole del solito — Gitea e Vikunja non sono il
> dominio di nessuno, sono due applicazioni pubbliche montate apposta — ma non
> sparisce: **è il tool che giudica il proprio output**, ed è la cosa che il
> metodo del collaudo era disegnato per evitare.
>
> Una seconda riserva, detta per intero: durante questa stessa sessione avevo
> già letto singoli finding dei report di Gitea e Vikunja, mentre correggevo il
> difetto sull'evidenza dei match parziali. **Non** ho aperto
> `collaudo/misure/`, che è la condizione che il metodo pone; ma non ero cieco
> ai dati come lo sarebbe stato il fondatore.
>
> Il numero che ne esce è quindi **un innesco, non un voto** — che è come
> COLLAUDO.md chiede di leggerlo comunque — e va confermato da te prima di
> tarare qualsiasi cosa.

Il campione è stratificato per tipo e per applicazione, e **non** è
proporzionale a quanto ciascun tipo compare davvero: serve a capire quali
famiglie valgono, non a misurarne il volume. Nessuna statistica in questo
file, di proposito.

---

## 1. Shadow — gitea

- endpoint: `/collaudo/frontend/issues`
- confidenza: high · severità: medium
- perché: not present in the declared inventory

  giudizio: rumore
  perché: pagina della UI web di Gitea, non un endpoint API: il log contiene sia UI sia API e la specifica descrive solo /api/v1

## 2. Undetermined — vikunja

- endpoint: `/api/v1/user/settings/webhooks/{id}`
- confidenza: low · severità: low
- perché: declared in the spec but never observed in this log
- perché: the analysed traffic covers 0 day(s), less than the staleness window of 30 day(s): too short to call it a zombie

  giudizio: rumore
  perché: dichiarato e non visto su una finestra di 0 giorni: si ripete una volta per ogni endpoint dichiarato non colpito

## 3. Shadow — gitea

- endpoint: `/collaudo/log-parser/issues/{id}`
- confidenza: high · severità: medium
- perché: not present in the declared inventory

  giudizio: rumore
  perché: stessa famiglia del 1: UI web

## 4. Undetermined — gitea

- endpoint: `/api/v1/repos/{owner}/{repo}/commits/{ref}/statuses`
- confidenza: low · severità: low
- perché: declared in the spec but never observed in this log
- perché: the analysed traffic covers 0 day(s), less than the staleness window of 30 day(s): too short to call it a zombie

  giudizio: rumore
  perché: stessa famiglia del 2: finestra troppo corta

## 5. Undetermined — gitea

- endpoint: `/api/v1/repos/collaudo/billing-service/issues/{id}`
- confidenza: medium · severità: medium
- perché: no exact match in the spec; the closest declared path is /api/v1/repos/{owner}/{repo}/issues/comments, which has a variable segment where this endpoint has a fixed one
- perché: reported as undetermined on purpose: calling it known would hide an endpoint the spec does not describe

  giudizio: rumore
  perché: match parziale su owner/repo letterali: si ripete su ogni repository

## 6. Undetermined — gitea

- endpoint: `/api/v1/users/{username}/subscriptions`
- confidenza: low · severità: low
- perché: declared in the spec but never observed in this log
- perché: the analysed traffic covers 0 day(s), less than the staleness window of 30 day(s): too short to call it a zombie

  giudizio: rumore
  perché: stessa famiglia del 2

## 7. Known — gitea

- endpoint: `/api/v1/user/repos`
- confidenza: high · severità: low
- perché: path and methods both declared in the spec as /api/v1/user/repos

  giudizio: non so
  perché: un Known non e' qualcosa che si guarda o si ignora: e' l'inventario. La domanda non si applica

## 8. Shadow — gitea

- endpoint: `/api/v1/repos/collaudo/docs-portal/contents/config/app.yaml`
- confidenza: high · severità: medium
- perché: not present in the declared inventory

  giudizio: rumore
  perché: il parametro dichiarato {filepath} contiene barre e non puo' mai combaciare: falso positivo strutturale

## 9. Undetermined — vikunja

- endpoint: `/api/v1/{username}/avatar`
- confidenza: low · severità: low
- perché: declared in the spec but never observed in this log
- perché: the analysed traffic covers 0 day(s), less than the staleness window of 30 day(s): too short to call it a zombie

  giudizio: rumore
  perché: stessa famiglia del 2

## 10. Shadow — gitea

- endpoint: `/collaudo/frontend/issues/{id}`
- confidenza: high · severità: medium
- perché: not present in the declared inventory

  giudizio: rumore
  perché: stessa famiglia del 1: UI web

## 11. Undetermined — gitea

- endpoint: `/api/v1/repos/collaudo/frontend/branches`
- confidenza: medium · severità: medium
- perché: no exact match in the spec; the closest declared path is /api/v1/repos/{owner}/{repo}/branches, which has a variable segment where this endpoint has a fixed one
- perché: reported as undetermined on purpose: calling it known would hide an endpoint the spec does not describe

  giudizio: rumore
  perché: match parziale su owner/repo letterali

## 12. Undetermined — gitea

- endpoint: `/api/v1/users/alessandra`
- confidenza: medium · severità: medium
- perché: no exact match in the spec; the closest declared path is /api/v1/users/{username}, which has a variable segment where this endpoint has a fixed one
- perché: reported as undetermined on purpose: calling it known would hide an endpoint the spec does not describe

  giudizio: rumore
  perché: match parziale su uno username letterale: si ripete su ogni utente

## 13. Known — vikunja

- endpoint: `/api/v1/tasks/{id}/comments`
- confidenza: high · severità: low
- perché: path and methods both declared in the spec as /api/v1/tasks/{taskID}/comments

  giudizio: non so
  perché: Known: la domanda non si applica

## 14. Undetermined — gitea

- endpoint: `/api/v1/repos/collaudo/billing-service/git/commits/{id}`
- confidenza: medium · severità: medium
- perché: no exact match in the spec; the closest declared path is /api/v1/repos/{owner}/{repo}/git/commits/{sha}, which has a variable segment where this endpoint has a fixed one
- perché: reported as undetermined on purpose: calling it known would hide an endpoint the spec does not describe

  giudizio: rumore
  perché: match parziale su owner/repo letterali

## 15. Undetermined — gitea

- endpoint: `/api/v1/users/davide`
- confidenza: medium · severità: medium
- perché: no exact match in the spec; the closest declared path is /api/v1/users/{username}, which has a variable segment where this endpoint has a fixed one
- perché: reported as undetermined on purpose: calling it known would hide an endpoint the spec does not describe

  giudizio: rumore
  perché: match parziale su uno username letterale

## 16. Undetermined — gitea

- endpoint: `/api/v1/markdown/raw`
- confidenza: low · severità: low
- perché: declared in the spec but never observed in this log
- perché: the analysed traffic covers 0 day(s), less than the staleness window of 30 day(s): too short to call it a zombie

  giudizio: rumore
  perché: stessa famiglia del 2

## 17. Undetermined — vikunja

- endpoint: `/api/v1/tasks/{taskID}/relations`
- confidenza: low · severità: low
- perché: declared in the spec but never observed in this log
- perché: the analysed traffic covers 0 day(s), less than the staleness window of 30 day(s): too short to call it a zombie

  giudizio: rumore
  perché: stessa famiglia del 2

## 18. Undetermined — gitea

- endpoint: `/api/v1/repos/{owner}/{repo}/wiki/pages`
- confidenza: low · severità: low
- perché: declared in the spec but never observed in this log
- perché: the analysed traffic covers 0 day(s), less than the staleness window of 30 day(s): too short to call it a zombie

  giudizio: rumore
  perché: stessa famiglia del 2

## 19. Known — vikunja

- endpoint: `/api/v1/tasks/{id}`
- confidenza: high · severità: low
- perché: path and methods both declared in the spec as /api/v1/tasks/{id}

  giudizio: non so
  perché: Known: la domanda non si applica

## 20. Undetermined — vikunja

- endpoint: `/api/v1/tasks/all`
- confidenza: medium · severità: medium
- perché: no exact match in the spec; the closest declared path is /api/v1/tasks/{id}, which has a variable segment where this endpoint has a fixed one
- perché: reported as undetermined on purpose: calling it known would hide an endpoint the spec does not describe

  giudizio: utile
  perché: 'all' e' una parola letterale dove la specifica ha un identificatore: e' il caso per cui §P3 esiste, ed e' l'unico finding del campione su cui aprirei davvero il codice

## 21. Undetermined — gitea

- endpoint: `/api/v1/users/chiara/repos`
- confidenza: medium · severità: medium
- perché: no exact match in the spec; the closest declared path is /api/v1/users/{username}/repos, which has a variable segment where this endpoint has a fixed one
- perché: reported as undetermined on purpose: calling it known would hide an endpoint the spec does not describe

  giudizio: rumore
  perché: match parziale su uno username letterale

## 22. Shadow — gitea

- endpoint: `/api/v1/repos/collaudo/log-parser/contents/config/app.yaml`
- confidenza: high · severità: medium
- perché: not present in the declared inventory

  giudizio: rumore
  perché: stessa famiglia dell'8: {filepath} con barre

## 23. Shadow — gitea

- endpoint: `/collaudo/frontend/src/branch/main/README.md`
- confidenza: high · severità: medium
- perché: not present in the declared inventory

  giudizio: rumore
  perché: stessa famiglia del 1: UI web

## 24. Known — gitea

- endpoint: `/api/v1/version`
- confidenza: high · severità: low
- perché: path and methods both declared in the spec as /api/v1/version

  giudizio: non so
  perché: Known: la domanda non si applica

## 25. Shadow — gitea

- endpoint: `/api/v1/repos/collaudo/infra-tooling/contents/config/app.yaml`
- confidenza: high · severità: medium
- perché: not present in the declared inventory

  giudizio: rumore
  perché: stessa famiglia dell'8

## 26. Known — vikunja

- endpoint: `/api/v1/projects/{id}/views`
- confidenza: high · severità: low
- perché: path and methods both declared in the spec as /api/v1/projects/{project}/views

  giudizio: non so
  perché: Known: la domanda non si applica, ma vale la pena annotare che {id} e {project} sono stati riconosciuti come lo stesso posto

## 27. Undetermined — gitea

- endpoint: `/api/v1/user/gpg_keys/{id}`
- confidenza: low · severità: low
- perché: declared in the spec but never observed in this log
- perché: the analysed traffic covers 0 day(s), less than the staleness window of 30 day(s): too short to call it a zombie

  giudizio: rumore
  perché: stessa famiglia del 2

## 28. Undetermined — vikunja

- endpoint: `/api/v1/user/settings/totp/enable`
- confidenza: low · severità: low
- perché: declared in the spec but never observed in this log
- perché: the analysed traffic covers 0 day(s), less than the staleness window of 30 day(s): too short to call it a zombie

  giudizio: rumore
  perché: stessa famiglia del 2

## 29. Known — vikunja

- endpoint: `/api/v1/register`
- confidenza: high · severità: low
- perché: path and methods both declared in the spec as /api/v1/register

  giudizio: non so
  perché: Known: la domanda non si applica

## 30. Undetermined — vikunja

- endpoint: `/api/v1/user/settings/totp/disable`
- confidenza: low · severità: low
- perché: declared in the spec but never observed in this log
- perché: the analysed traffic covers 0 day(s), less than the staleness window of 30 day(s): too short to call it a zombie

  giudizio: rumore
  perché: stessa famiglia del 2

