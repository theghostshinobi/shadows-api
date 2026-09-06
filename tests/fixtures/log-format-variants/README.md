# Le sette varianti di `log_format`

Copia **byte per byte** di `collaudo/corpus/varianti-log-format/`, ricostruite
dai formati documentati durante il collaudo del 2026-08-30 (COLLAUDO.md, §4).

Stanno qui perché §8 vuole il corpus dorato sotto `tests/fixtures/`, e restano
legate all'originale da un test che confronta i byte: se le due copie divergono,
la fixture smette di presidiare la misura da cui è nata.

| File | `log_format` che lo produce | Prima della taratura |
|---|---|---|
| `01-nginx-combined.log` | `combined` di nginx | **accettato** |
| `02-main-con-xforwardedfor.log` | `main` dell'esempio di nginx | rifiutato |
| `03-combined-con-request-time.log` | `combined` + `$request_time` | rifiutato |
| `04-combined-con-request-e-upstream-time.log` | `combined` + `$request_time` + `$upstream_response_time` | rifiutato |
| `05-ingress-nginx-default.log` | default di ingress-nginx (nove campi in più) | rifiutato |
| `06-apache-common.log` | `common` di Apache (senza referer e user agent) | rifiutato |
| `07-request-time-in-testa.log` | `$request_time` in testa alla riga | rifiutato |
