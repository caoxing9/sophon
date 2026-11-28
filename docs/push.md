# Push notifications

A paired iPhone (and through it an Apple Watch) hears when an agent on the
computer needs its owner: it waits for an approval, asks a question,
finished or failed its turn, or the plan's limit is near. An approval is
answered from the notification, and any of them can be replied to.

```
Sophon service (each space's) ──ws──▶ push gateway (sophon-relay /push) ──HTTP/2──▶ APNs ──▶ iPhone ──▶ Watch
        ▲                                   │
        └──────────── act ◀─────────────────┘◀──ws── iPhone (an action pressed, the app not open)
```

- **The service decides and sends**, whether the app is open or not: it
  runs the agents, so it knows at once when one asks or ends
  (`crates/sophon-service/src/push.rs`). The shared part (sealing, who
  wants what, the words, the gateway's socket) is `crates/sophon-push`.
- **The app keeps who wants them.** A phone registers over the app's
  bridge; the app keeps the record in its data folder (`push.json`) and
  writes each space's share of it into that space's service home
  (`push.json` there, below), at once on a change and every ten seconds.
- The gateway holds the one thing the computer cannot: the APNs signing key
  of the app. It is part of `sophon-relay`, on the same port.
- Apple shows a notification's text itself, so the gateway and Apple read
  it. What travels to the gateway is sealed to the gateway's key, so
  nothing else on the way does. "Hide content" (a setting) sends the state
  alone ("Waiting for your approval", "3 agents finished").

## When, and what it says

- An approval or a question: at once. Title: the conversation. Body: the
  state, what it asks (the command, the file, the question) and the
  project: `等待你的确认：npm test · sophon`.
- A turn that finished or failed: five seconds after it ended, unless a
  turn of that agent began meanwhile (a follow-up), with the reply's first
  words (Markdown's marks and headings left out, 200 characters at most)
  or the error: `已完成 · sophon⏎All 42 tests pass now.`
- Turns done within the same second are one push ("3 agents finished",
  their names under it; `thread`/`collapse` `finished`).
- Nothing for a turn the user stopped, an agent another started, an
  automation's run that finished as it should (a failed one is said), or
  anything before the service started.
- **Someone at the Mac:** a turn's end is not pushed while Sophon is the
  app in front there and the Mac was used in the last five minutes. The
  app says whether it is in front as that changes and every 30 s
  (`sophon.presence.request {front}`; a word holds 90 s); the service asks
  macOS how long since the last key or pointer event. Approvals and
  questions are pushed all the same.
- A plan's limit passing 80% and 95%: the app watches the plans and asks
  the service to push it (`sophon.push.usage.request {label, percent, ends}`).
- Each kind can be turned off on the phone (Turn finished, Approvals,
  Questions and errors, Plan limits): `events` in its registration.

## The service's `push.json`

Written by the app into each service home that has devices (removed when
none is left), the user's alone (0600):

```json
{ "id": "<this space's id at the gateway>", "gateway": "host:port", "tls": false,
  "devices": [{ "token": "…", "env": "production", "lang": "zh", "hide": false,
                "pairing": "<token>", "space": null,
                "events": { "finished": true, "permission": true, "attention": true, "usage": true } }],
  "secret": "<the bridge's secret key, base64>", "tokens": ["<pairing tokens good now>"] }
```

The first space's id is the computer's (`push.json` in the app's data
folder); another space's is that id, `-` and a hash of its home, so each
service has a socket of its own at the gateway. A phone is told the id of
the space it registered for (`sophon_push_info`). The service keeps the
gateways' keys as it first saw them in `push-gateways.json`.

A device that forgot the computer (`forget`, below) comes off the
service's list at once and is written to `push-forgotten.json` in its
home; the app takes it off its record, revokes its own pairing, and
clears the file.

## Sealing

A sealed value is `base64(ephemeral public key (32) ‖ nonce (24) ‖ box)`:
NaCl `crypto_box` (X25519, XSalsa20-Poly1305) from a keypair made for the
one message to the receiver's public key.

- To the gateway: its public key, which it says first on every socket
  (`hello`). A computer keeps the key it first saw for a gateway and
  refuses another.
- To the computer (an action): the bridge's public key, which the pairing
  link gave the device. The sealed JSON carries the device's pairing
  token, which is what the computer checks: the key alone lets anyone
  seal, the token says who did.

## Gateway: `GET /push` (WebSocket), JSON text frames

Query: `id=<computer id>` — the computer's socket (one per id; a newer one
replaces it) — or `to=<computer id>` — a device's socket, to send actions.
The computer id is random, 16+ characters of `[A-Za-z0-9_-]`.

Gateway → every socket, first:

```json
{ "type": "hello", "key": "<base64 of the gateway's 32-byte public key>", "apns": true }
```

`apns` is false when the gateway has no APNs key: it then answers every
push with `"reason": "NotConfigured"`.

Computer → gateway:

```json
{ "type": "push", "ref": 7, "sealed": "<sealed JSON, below>" }
{ "type": "ping" }
```

```json
{
  "token": "<APNs device token, hex>",
  "env": "production" | "development",
  "title": "sophon · main",
  "body": "Waiting for your approval: npm test",
  "category": "PERMISSION" | "FINISHED" | "ATTENTION" | "USAGE",
  "thread": "<agent id, groups them>",
  "collapse": "<replaces an earlier one with the same id, optional>",
  "data": { "...": "the app's own fields, copied into the payload under `sophon`" }
}
```

Gateway → computer: `{ "type": "pushed", "ref": 7, "ok": true }` or
`{ ..., "ok": false, "reason": "BadDeviceToken" }` (APNs' reason, or
`NotConfigured`, `BadRequest`, `RateLimited`, `Unreachable`); `{ "type": "pong" }`.

The APNs request: `POST https://api.push.apple.com/3/device/<token>`
(`api.sandbox.push.apple.com` for `development`), `apns-topic: dev.sophon.app`,
`apns-push-type: alert`, `apns-priority: 10`, `apns-collapse-id` when given,
`authorization: bearer <ES256 JWT: iss team id, iat, kid key id>` (kept for
50 minutes), body:

```json
{ "aps": { "alert": { "title": "…", "body": "…" }, "sound": "default",
           "category": "…", "thread-id": "…", "interruption-level": "time-sensitive" },
  "sophon": { "…": "…" } }
```

`interruption-level` is `time-sensitive` for `PERMISSION` only.

Device → gateway (`to=`): `{ "type": "act", "ref": 1, "sealed": "<sealed to the computer>" }`.
The gateway passes it to the computer's socket as
`{ "type": "act", "from": <socket number>, "ref": 1, "sealed": "…" }`; the
computer answers `{ "type": "acted", "from": <socket number>, "ref": 1, "ok": true }`,
which the gateway passes back without `from`. No computer socket:
`{ "type": "acted", "ref": 1, "ok": false, "reason": "Offline" }`.

Limits: a frame is at most 16 KB; a socket sends at most 60 pushes a
minute; one device token receives at most 30 a minute.

Configuration (environment): `APNS_KEY_FILE` (the `.p8`), `APNS_KEY_ID`,
`APNS_TEAM_ID`, `APNS_TOPIC` (default `dev.sophon.app`), `RELAY_STATE_DIR`
(default `/var/lib/sophon-relay`: the gateway's own key, `push.key`, made
on the first start).

## Computer and device

Over the bridge (already encrypted), after the device authenticated:

- Device → computer: `{ "type": "sophon_push_register", "device": "<its own id>", "token": "<hex>", "env": "production", "lang": "zh", "hide": false, "events": { "finished": true, "permission": true, "attention": true, "usage": true } }`
  (no `events`, an older app: all of them)
  (an empty token: it no longer wants any).
- Computer → device: `{ "type": "sophon_push_info", "gateway": "host:port", "id": "<computer id>" }`,
  which the device keeps to send actions.

An action, sealed to the computer:

```json
{ "token": "<the device's pairing token>", "at": 1794450000,
  "kind": "permission", "agent": "<id>", "permission": "<id>", "allow": true }
{ "token": "…", "at": 1794450000, "kind": "reply", "agent": "<id>", "text": "run the tests" }
{ "token": "…", "at": 1794450000, "kind": "forget", "device": "<the id it registered with>" }
```

`at` (seconds) older than ten minutes is refused.

`forget` is sent when the phone forgets the computer: the computer takes
that device off the push list (only when it registered with the same
pairing token) and revokes the pairing when the token was the device's
own (not the QR's or a space's). Until the computer answers `ok` (or
refuses the token: already revoked), the phone keeps it as forgotten,
sends `forget` again at each launch and when one of its notifications
arrives, and does not show those notifications while the app is open.
Revoking a device on the computer (Remove, Unpair all) also takes it off
the push list at once.

The notification's `sophon` data: `{ "host": "<computer id>", "agent": "<id>", "permission": "<id>" }`.
