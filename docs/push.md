# Push notifications

A paired iPhone (and through it an Apple Watch) hears when an agent on the
computer needs its owner: it waits for an approval, finished its turn, or
the plan's limit is near. An approval is answered from the notification.

```
computer (Sophon, macOS) ──ws──▶ push gateway (sophon-relay /push) ──HTTP/2──▶ APNs ──▶ iPhone ──▶ Watch
        ▲                                   │
        └──────────── act ◀─────────────────┘◀──ws── iPhone (an action pressed, the app not open)
```

- The computer decides when to notify: it already watches its agents and
  knows the devices paired with it.
- The gateway holds the one thing the computer cannot: the APNs signing key
  of the app. It is part of `sophon-relay`, on the same port.
- Apple shows a notification's text itself, so the gateway and Apple read
  it. What travels to the gateway is sealed to the gateway's key, so
  nothing else on the way does. "Hide content" (a setting) sends a plain
  "needs your attention" instead.

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

- Device → computer: `{ "type": "sophon_push_register", "token": "<hex>", "env": "production", "lang": "zh", "hide": false }`
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
