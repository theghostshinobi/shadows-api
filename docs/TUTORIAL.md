# Shadow, step by step

Twenty minutes, from a log file to an inventory you can hand to an auditor.
Everything here runs on your machine, offline.

---

## 0. Before you start

You need a web server access log and, ideally, the OpenAPI spec of the service
behind it. Without the spec Shadow still works, but it can only **suspect** —
it cannot verify absence from an inventory that does not exist.

```bash
cargo build --release
alias shadow=./target/release/shadow
```

---

## 1. The first run

```bash
shadow /var/log/nginx/access.log
```

If your log is nginx's `combined`, you get a list of observed endpoints. If it
is not, you get this instead:

```
shadow: access.log does not look like the 'nginx-combined' log format:
        100 non-empty line(s) examined, none could be parsed (line 1: malformed-line)
  expected each line to look like: <remote_addr> - <remote_user> [<time>] "<method> <path> HTTP/<version>" …
  declare the right format with --log-format, or check that this is the file you meant
```

**This is the tool working, not failing.** Reading fields from the wrong
positions would give you numbers that look real and are not.

---

## 2. Declare your log format

Most installations add fields. Copy the `log_format` line out of your server
configuration and pass it:

```bash
shadow access.log --log-format '$remote_addr - $remote_user [$time_local] "$request" $status $body_bytes_sent "$http_referer" "$http_user_agent" $request_time'
```

The declared grammar is validated just as strictly as the built-in one: a line
that does not match it is discarded and counted, never read field by field from
the wrong places.

If the log carries both your web pages and your API, tell Shadow which part is
the API — otherwise every page looks like an undocumented endpoint:

```bash
shadow access.log --path-prefix /api/v1
```

Requests outside the prefix are **counted, not hidden**. Shadow will not decide
for you which traffic is the API.

---

## 3. Add the declared inventory

```bash
shadow access.log --openapi-spec openapi.json
```

Now the verdicts mean something:

```
findings (22)
  Shadow        high    severity high   a41c0f22…  /api/v1/admin/reset
      because not present in the declared inventory
      because it answers without observed authentication

  Undetermined  medium  severity medium 5f21ab90…  /api/v1/tasks/all
      because no exact match in the spec; the closest declared path is
              /api/v1/tasks/{id}, which has a variable segment where this
              endpoint has a fixed one
      because reported as undetermined on purpose: calling it known would hide
              an endpoint the spec does not describe
```

That second one is the case worth your time: `all` is a **word** sitting where
the spec expects an identifier.

**Exit codes**, so this fits in CI:

| Code | Meaning |
|---|---|
| `0` | Analysis completed, no `Shadow` / `Zombie` |
| `1` | Execution error (unreadable input, unrecognised format) |
| `2` | Usage error |
| `3` | Analysis completed **and** confirmed findings — fail the build |

Uncertain findings alone do not fail a build. If you want them to:
`--fail-on-undetermined`, opt-in and never the default.

---

## 4. Silence the noise you have already judged

```bash
shadow access.log --openapi-spec openapi.json --allowlist known-noise.allow
```

An allowlisted finding disappears from the report but **stays in the manifest
counts**. Silencing is a decision you can review later, not an erasure. Shadow
warns you when a rule is broad enough to hide more than you meant.

---

## 5. Remember, so you can tell new from old

```bash
shadow access.log --openapi-spec openapi.json \
      --history shadow.db --target payments-api
```

The second run on the same input says nothing new. The run after a deploy tells
you exactly what appeared:

```
shadow: 2 endpoint(s) never seen before on target 'payments-api':
shadow: new  a41c0f22…  /api/v1/admin/reset          (Shadow)
shadow: new  7b3e91d4…  /api/v1/internal/debug/dump  (Shadow)
```

Different targets never mix. One daemon can watch several services, and their
inventories stay separate.

---

## 6. Watch over time

```bash
shadow daemon access.log --openapi-spec openapi.json \
      --history shadow.db --target payments-api \
      --interval 60 --dashboard /var/www/shadow.html
```

It reads only what has grown, but always judges everything it has seen — so its
verdict is the one a single run would give on the file so far.

`--dashboard` rewrites a self-contained HTML page after every cycle. Leave it
open in a browser: the page re-reads itself, and **nothing is listening on a
port**.

A rotated or truncated log is detected and said out loud, never silently
restarted.

---

## 7. Look at it

```bash
shadow status    --history shadow.db --target payments-api   # terminal
shadow dashboard --history shadow.db --out state.html        # a page you can send
shadow serve     --history shadow.db                         # http://127.0.0.1:8787
```

`shadow serve` is the only command in Shadow that opens a socket. It listens on
this machine only unless you say otherwise, it is read-only, and it tells you
out loud if you bind it somewhere reachable from outside.

On macOS, `sh macos/build.sh` builds a menu-bar app that shows the same view.
Point it at your history in
`~/Library/Application Support/Shadow/menubar.json`.

---

## 8. Deal with what turned up

```bash
shadow alerts --history shadow.db --target payments-api
```

```
2 unacknowledged shadow endpoint(s) on target 'payments-api'

  a41c0f22…  /api/v1/admin/reset
      first seen in run 12, last seen in run 41
```

Once you have looked at one:

```bash
shadow alerts --history shadow.db --acknowledge a41c0f22…
```

Acknowledging removes it from the queue and **keeps the evidence**. An alert is
a fact that happened; a later run that has nothing to say about that endpoint
cannot erase it.

---

## 9. Hand it to whoever runs the review

```bash
shadow compliance --history shadow.db --target payments-api --format csv > inventory.csv
shadow compliance --history shadow.db --target payments-api --format markdown > inventory.md
shadow compliance --history shadow.db --target payments-api --format json > inventory.json
```

The CSV is what an auditor opens. The Markdown is the readable version, with the
limits stated up front. The JSON is a versioned contract, `shadow-compliance/1`.

All three carry the **four** authentication values and the number of observable
requests each verdict rests on. None of them collapses those four into a tick
box, and all three say what the document is not:

> Shadow is a discovery aid, not a security certification. This inventory covers
> only endpoints that appeared in the analysed logs.

---

## Where things get interesting

- **`--show-raw-values`** turns redaction off. The report then says so about
  itself, in every format, so a raw report can never be mistaken for a redacted
  one that happened to have nothing to hide.
- **`zombie_staleness_days`** decides how long silence means "gone". On a log
  shorter than that window, Shadow says *I cannot tell* instead of guessing.
- **Every setting** can come from a flag, an environment variable
  (`SHADOW_` + the key name in capitals), or the default — and the manifest
  records **which one it came from**. "90 days because of a config file nobody
  remembered" is audit information, not a detail.
