<div align="center">

# ≡ SHADOWS — API

### Shadow API discovery from your web server logs

**Find the API endpoints that answer but nobody documented.**

Point it at an nginx or Apache access log. It tells you which endpoints are
actually responding that are **not** in your OpenAPI spec — *shadow endpoints* —
and which are documented but no longer answer — *zombie endpoints*.

Offline by design. It never opens a network connection, it never phones home,
and your logs never leave the machine.

`shadow API detection` · `undocumented endpoints` · `API inventory` ·
`OpenAPI drift` · `zombie endpoints` · `nginx log analysis` · `PCI DSS 4.0
endpoint inventory` · `Rust` · `CLI`

</div>

---

## Who this is for

- You have an OpenAPI spec and a suspicion that reality has drifted from it.
- You need an **endpoint inventory** for an audit — PCI DSS 4.0 asks for one —
  and you would rather derive it from real traffic than from a wiki page.
- You run an API gateway and want to know what appeared after last Thursday's
  deploy, without installing an agent or sending traffic anywhere.
- Someone asked *"are we sure nothing undocumented is exposed?"* and you want a
  better answer than *"probably"*.

---

## What it is

```
$ shadow /var/log/nginx/access.log --openapi-spec openapi.json

observed endpoints (17)
  8c6b9716…  GET   /api/v1/users/{id}                39  auth: not observable
  a41c0f22…  POST  /api/v1/admin/reset                3  auth: no (based on 3 of 3 observable)
  …

findings (22)
  Shadow        high    severity high   a41c0f22…  /api/v1/admin/reset
      because not present in the declared inventory
      because it answers without observed authentication
  …
```

Exit code `3`, so a CI pipeline can fail on purpose.

## What it is **not**

This list matters as much as the one above, and the tool says it about itself in
every report it produces.

- **Not a security certification.** It is a discovery aid. It tells you what to
  go and look at.
- **Not runtime protection.** It does not block traffic. It is not a WAF.
- **Not a scanner.** It generates no traffic towards your systems. It reads logs
  that already exist.
- **Not a cloud service.** It does not phone home. There is no telemetry.

## Four verdicts, and one of them is "I don't know"

| Verdict | Meaning |
|---|---|
| **Shadow** | Observed in traffic, absent from the declared inventory |
| **Zombie** | Declared, but not observed for longer than the staleness window |
| **Known** | Observed and matches a declared endpoint |
| **Undetermined** | Uncertain or partial match — **a first-class result, never quietly promoted to Known** |

A wrong `Known` is an endpoint nobody will ever look at again. A wrong
`Undetermined` is an endpoint somebody looks at once too often. The tool is
built around that asymmetry.

The same discipline governs **authentication**, which carries **four** values and
never collapses into present/absent:

`yes` · `no` · `mixed` · `not observable`

`not observable` means the log format does not carry the information. It is
**not** evidence that authentication was missing — and next to it you always
get the number of observable requests the verdict rests on, because
*"no auth observed"* and *"no auth observed, on 3 requests out of 5000"* are not
the same statement.

## The four ways to look at it

<div align="center">
<img src="docs/images/menubar.png" width="520" alt="Shadow in the macOS menu bar">
</div>

| | Command |
|---|---|
| **Terminal** | `shadow status` |
| **Self-contained page** | `shadow dashboard --out state.html` |
| **Local server** | `shadow serve` |
| **macOS menu bar** | `Shadow.app` |

All four show the **same view**. They cannot disagree, because they read the
same structure.

<div align="center">
<img src="docs/images/dashboard.png" width="720" alt="The Shadow dashboard">
</div>

## Getting started

```bash
# analyse a log once
shadow access.log --openapi-spec openapi.json

# your server adds fields to the log format? declare it — it never guesses
shadow access.log --log-format '$remote_addr - $remote_user [$time_local] "$request" $status $body_bytes_sent "$http_referer" "$http_user_agent" $request_time'

# remember what you have seen, and watch over time
shadow daemon access.log --openapi-spec openapi.json --history shadow.db --interval 60

# what appeared and nobody has looked at yet
shadow alerts --history shadow.db

# the endpoint inventory, for whoever runs the review
shadow compliance --history shadow.db --format csv > inventory.csv
```

There is a step-by-step walkthrough in **[docs/TUTORIAL.md](docs/TUTORIAL.md)**.

## Building

Rust 1.85 or newer, and nothing else. No network access is needed at build time
or at run time.

```bash
cargo build --release          # -> target/release/shadow
sh macos/build.sh              # -> macos/build/Shadow.app  (macOS only)
```

## The rules it will not break

These are not aspirations. Each one is enforced by tests that fail if it stops
being true.

- **It fails loudly, never quietly wrong.** If the log format does not match, it
  refuses to read the file rather than pull fields from the wrong positions. A
  plausible-but-invented number is worse than an error.
- **Redaction is on by default**, everywhere it prints or persists — report,
  manifest, history, dashboard. Logs carry tokens in URLs and emails in query
  strings, and a report gets pasted into shared tickets.
- **Same input, same verdict.** Including with a daemon running: time, arrival
  order and accumulated state never change what it concludes.
- **It never merges what it cannot prove is the same.** `admin` does not get
  swallowed into `{id}`, no matter how many identifiers surround it.
- **No manifest, no verdict.** An invocation that analyses nothing never claims
  "no findings".
- **One socket, in one file.** Only `shadow serve` listens, only when you ask,
  only on `127.0.0.1` unless you say otherwise, and read-only.

## Status

Pre-1.0, and honest about it. The analysis pipeline and the persistence layer
are complete and tested; the licence text still has placeholders, and the name
is provisional.

## Licence

Business Source License 1.1 — free for individual use, paid for commercial use,
converting to an open licence after four years.

> **The licence text is not finished.** Several parameters are still
> placeholders. Until they are filled in, treat this repository as source-
> available for inspection, not as licensed for use.

## A note on languages

Everything a user reads is in **English**: the command line, the reports, the
evidence, this README. The project's own design documents — the blueprint, the
progress log, the field-test notes — are in **Italian**, and stay that way: they
are a conversation between the people building it.
