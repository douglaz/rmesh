# Changelog

## Unreleased

### Changed — affects scripts parsing `--json`

- **LoRa region names are now the canonical protobuf spellings.** `rmesh` previously
  emitted its own shortened forms; it now prints what the protobufs define, which is also
  what the reference Meshtastic client and the firmware use.

  | before | now |
  |---|---|
  | `EU433` | `EU_433` |
  | `EU868` | `EU_868` |
  | `NZ865` | `NZ_865` |
  | `LORA24` | `LORA_24` |
  | `UA433` / `UA868` | `UA_433` / `UA_868` |
  | `MY433` / `MY919` | `MY_433` / `MY_919` |
  | `SG923` | `SG_923` |
  | `PH433` / `PH868` / `PH915` | `PH_433` / `PH_868` / `PH_915` |
  | `Unset` | `UNSET` |

  `US`, `ANZ`, `CN`, `JP`, `KR`, `TW`, `RU`, `IN` and `TH` are unchanged. Affects
  `info radio`, `config get lora.region` and `config list`, in both table and JSON output.

  `config set lora.region` still accepts the old spellings, so existing *write* commands
  keep working; only code matching on the *printed* string needs updating. This also closes
  a gap where `MY433`, `SG923`, `LORA24`, `PH433`, `PH868` and `PH915` were printed but
  rejected on input.

- A hardware model, or a region, that is newer than the protobufs this build was compiled
  against now prints as `Unknown(<id>)` rather than `Unset`/`UNSET`. Those are real values
  meaning "not set", so reporting them for something merely unrecognised was misleading.

### Fixed — channel commands

- **`channel add`, `channel set` and `channel delete` now work.** All three were unfinished
  and destructive, and were inert only because admin packets were addressed to node 0 and
  discarded by the radio; correcting the address would have armed them.
  - `channel add` wrote index 0 with role PRIMARY — indices are explicit, not allocated by
    the radio — so every add overwrote the primary channel, name and PSK included. It now
    allocates the lowest disabled slot and writes role SECONDARY.
  - `channel set` rebuilt the channel from defaults, so changing only `--name` erased the
    PSK and forced role PRIMARY. It now reads the channel from the radio and changes only
    the fields given, preserving the role.
  - `channel delete N` sent `RemoveByNodenum(N)`, a NodeDB operation: the channel stayed
    and a node numbered N was removed instead. It now disables channel N, refuses index 0,
    and reports a channel that is already disabled.
- PSKs are decoded as base64 (or `none` to clear) rather than taken as raw bytes, which
  silently produced an invalid key.

### Fixed

- `info radio` reported the firmware version from `min_app_version` (the minimum *client
  app* version), so a radio on 2.7.26 displayed `3.2.0`.
- `info radio` and `info nodes` reported `Unset` for any board the vendored protobufs did
  not know, and `rmesh-test` could report a *neighbour's* hardware model instead of the
  connected radio's.
- `config set` built the outgoing message from defaults, which would have blanked every
  field in the same config group that the caller did not name (for `lora.region`:
  `tx_power`, `hop_limit`, `channel_num`, `modem_preset`). It now edits the radio's current
  config, and refuses if it has not received one.
- `config set` reported success whenever the packet was written to the port, whether or not
  the radio applied it. It now reads the value back and fails if it did not change.
- `config set lora.region UNSET` is refused: it stops the radio transmitting.
- `config set device.role` accepts `ROUTER_LATE` and `CLIENT_BASE`, and warns when setting
  a role upstream deprecated for harming public meshes.
- `connect()` waited a fixed 500 ms before reading device state instead of waiting for the
  configuration dump to finish, and issued seven admin requests the radio always rejected,
  costing ~1.7 s per connection.
