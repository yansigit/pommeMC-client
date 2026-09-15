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

Wave 0 adds executable scenario manifests here. Each scenario records its
protocol, server fixture, account mode, bounded timeout, command, expected
checkpoints, and retained artifacts. The acceptance runner writes one result
record per scenario and never treats compilation alone as behavioral evidence.
