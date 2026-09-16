# Acceptance and gap ledger

This directory contains machine-readable inputs for closing the gaps in
[`docs/GAP_ROADMAP.md`](../docs/GAP_ROADMAP.md).

## Gap ledger

`gap-ledger.json` tracks two things:

- the 18 roadmap-level requirements and their implementation waves;
- every explicit first-party Rust TODO/FIXME-style source marker present at the
  baseline revision.

Validate it before committing implementation work:

```sh
python3 tools/gap_ledger.py check
```

The check fails when a new source marker is introduced without being tracked,
when an open marker disappears without closure evidence, when a closed marker
still exists, or when a requirement is marked closed without evidence.

Only regenerate the ledger deliberately after reconciling existing statuses and
evidence:

```sh
python3 tools/gap_ledger.py generate
```

Regeneration resets entries to `open`, so it is a bootstrap and recovery command,
not the normal way to close work. Closing a gap requires adding its tests and
public-path acceptance artifact to the entry's `evidence` list.

## Scenario manifests

`scenarios.json` is executed by the `pomme-acceptance` runner. Each scenario records its
protocol, server fixture, account mode, bounded timeout, command, expected
checkpoints, and retained artifacts. The acceptance runner writes one result
record per scenario and never treats compilation alone as behavioral evidence.

Build the runner before invoking it so its child `cargo` commands do not contend
with the parent process for Cargo locks:

```sh
cargo build -p pomme-acceptance
target/debug/pomme-acceptance
```

Use `--list` to inspect the matrix or repeat `--scenario <id>` to run a focused
subset. Results, stdout, stderr, and validated retained artifacts are written to
`target/acceptance/<run-id>/<scenario-id>`. Temporary game and world directories
are deleted unless `--keep-temp` is supplied.

## Scripted client actions

`scripts/*.json` drives the client without GUI automation. Invoke one with
`--automation-script <path> --automation-trace <path>`. Scripts can wait for an
application phase, press or release a named `Action`, wait a bounded number of
frames, assert observable UI state, and exit. Presses are folded into
`InputState` and consumed by the same frame handlers as keyboard, mouse, and
controller input. The client never mutates the asserted gameplay state directly.

The trace records every dispatched action and assertion. A timeout, failed
assertion, malformed script, or application exit before the explicit `exit`
step produces a non-zero client result and a failure trace.
