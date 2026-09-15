#!/usr/bin/env python3
"""Validate or regenerate Pomme's executable implementation-gap ledger."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LEDGER = ROOT / "acceptance" / "gap-ledger.json"
MARKER_RE = re.compile(r"TODO|FIXME|HACK|XXX|todo!\(|unimplemented!\(")
IGNORED_PARTS = {".git", "target", "third_party"}


def normalize(text: str) -> str:
    return " ".join(text.strip().split())


def wave_for(path: str) -> tuple[str, str]:
    if path.startswith("pomme-protocol/") or "/net/translate.rs" in path or "/net/handler.rs" in path:
        return "2-3", "protocol-compatibility"
    if any(part in path for part in ("/physics/", "/world/block/", "/player/")):
        return "4", "gameplay-state-and-physics"
    if any(
        part in path
        for part in (
            "/renderer/",
            "/ui/",
            "/audio",
            "/particle",
            "/app/",
        )
    ):
        return "5", "presentation-and-interface"
    if path.startswith("pomme-singleplayer/"):
        return "6", "integrated-server"
    return "hardening", "maintenance"


def source_markers() -> list[dict[str, object]]:
    discovered: list[tuple[str, int, str]] = []
    for source in sorted(ROOT.rglob("*.rs")):
        relative = source.relative_to(ROOT)
        if any(part in IGNORED_PARTS for part in relative.parts):
            continue
        for line_number, line in enumerate(source.read_text(errors="replace").splitlines(), 1):
            if MARKER_RE.search(line):
                discovered.append((relative.as_posix(), line_number, normalize(line)))

    duplicate_index: Counter[tuple[str, str]] = Counter()
    markers: list[dict[str, object]] = []
    for path, line, text in discovered:
        key = (path, text)
        occurrence = duplicate_index[key]
        duplicate_index[key] += 1
        digest = hashlib.sha1(f"{path}\0{text}\0{occurrence}".encode()).hexdigest()[:12]
        wave, area = wave_for(path)
        markers.append(
            {
                "id": f"marker-{digest}",
                "path": path,
                "line_hint": line,
                "text": text,
                "occurrence": occurrence,
                "wave": wave,
                "area": area,
                "status": "open",
                "evidence": [],
            }
        )
    return markers


def git_head() -> str:
    return subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
    ).strip()


def requirement_entries() -> list[dict[str, object]]:
    rows = [
        ("acceptance-runner", "0", "acceptance", "Executable scenario manifests, bounded process orchestration, traces, and artifacts"),
        ("native-776", "1", "protocol", "Validated native 26.2 login-to-gameplay behavior"),
        ("legacy-translation", "2-3", "protocol", "Lossless supported 1.21.x and 1.20.x behavior translation"),
        ("authoritative-client-state", "4", "state", "Central abilities, attributes, effects, pose, equipment, and passenger state"),
        ("generated-block-shapes", "4", "physics", "Generated collision and outline data without unsupported full-cube fallbacks"),
        ("movement-interaction-combat", "4", "gameplay", "Movement, fluids, interaction rollback, mining, placement, and combat parity"),
        ("inventory-semantics", "5", "inventory", "Server-authoritative semantics for every supported menu and creative action"),
        ("fluid-rendering", "5", "rendering", "Tinted, height-aware, directional fluids with correct translucent ordering"),
        ("block-entity-rendering", "5", "rendering", "Complete state and presentation for supported block entities"),
        ("entity-presentation", "5", "rendering", "Entity models, layers, equipment, interpolation, and debug hitboxes"),
        ("particles-audio-assets", "5", "rendering", "Payload-complete particles, audio behavior, and asset-derived presentation"),
        ("hud-camera-input", "5", "interface", "HUD, camera effects, input, controller, spectator, and accessibility parity"),
        ("steel-spawning", "6", "server", "Biome-aware natural spawning, caps, packs, despawn, and persistence"),
        ("steel-survival-substrate", "6-7", "server", "Stateful survival blocks, systems, portals, projectiles, and saved data"),
        ("steel-mobs-tier-1", "6", "server", "Shared hostile navigation, targeting, combat, drops, and despawn"),
        ("steel-mobs-tier-2-3", "7", "server", "Passive, neutral, and structured encounter mob families"),
        ("steel-bosses-progression", "8", "server", "Wither, Ender Dragon, progression, encounter persistence, and rewards"),
        ("platform-performance", "hardening", "platform", "Cross-platform packaging, dependency maintenance, and performance thresholds"),
    ]
    return [
        {
            "id": gap_id,
            "wave": wave,
            "area": area,
            "requirement": requirement,
            "status": "open",
            "evidence": [],
        }
        for gap_id, wave, area, requirement in rows
    ]


def generate() -> None:
    markers = source_markers()
    document = {
        "schema": 1,
        "source_revision": git_head(),
        "status_values": ["open", "in_progress", "blocked", "closed", "not_a_gap"],
        "closure_rule": "closed entries require evidence; open source markers must still exist",
        "requirements": requirement_entries(),
        "markers": markers,
        "summary": {
            "first_party_markers": len(markers),
            "client_markers": sum(
                marker["path"].startswith("pomme-client/") for marker in markers
            ),
        },
    }
    LEDGER.parent.mkdir(parents=True, exist_ok=True)
    LEDGER.write_text(json.dumps(document, indent=2) + "\n")
    print(f"wrote {LEDGER.relative_to(ROOT)} with {len(markers)} source markers")


def check() -> None:
    document = json.loads(LEDGER.read_text())
    if document.get("schema") != 1:
        raise SystemExit("unsupported gap-ledger schema")

    current = {marker["id"]: marker for marker in source_markers()}
    recorded = {marker["id"]: marker for marker in document["markers"]}
    errors: list[str] = []

    for marker_id, marker in recorded.items():
        status = marker["status"]
        evidence = marker.get("evidence", [])
        exists = marker_id in current
        if status in {"open", "in_progress", "blocked"} and not exists:
            errors.append(f"{marker_id} disappeared while status is {status}")
        if status in {"closed", "not_a_gap"} and not evidence:
            errors.append(f"{marker_id} is {status} without evidence")
        if status == "closed" and exists:
            errors.append(f"{marker_id} is closed but its source marker still exists")

    for marker_id, marker in current.items():
        if marker_id not in recorded:
            errors.append(
                f"untracked marker {marker_id} at {marker['path']}:{marker['line_hint']}"
            )

    for requirement in document["requirements"]:
        if requirement["status"] == "closed" and not requirement.get("evidence"):
            errors.append(f"requirement {requirement['id']} is closed without evidence")

    expected = document["summary"]
    if expected["first_party_markers"] != len(recorded):
        errors.append("summary first_party_markers does not match recorded markers")
    recorded_client = sum(
        marker["path"].startswith("pomme-client/") for marker in recorded.values()
    )
    if expected["client_markers"] != recorded_client:
        errors.append("summary client_markers does not match recorded markers")

    if errors:
        raise SystemExit("gap ledger check failed:\n- " + "\n- ".join(errors))

    print(
        "gap ledger ok: "
        f"{len(document['requirements'])} requirements, "
        f"{len(recorded)} source markers, {recorded_client} client markers"
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=["check", "generate"])
    args = parser.parse_args()
    if args.command == "generate":
        generate()
    else:
        check()


if __name__ == "__main__":
    main()
