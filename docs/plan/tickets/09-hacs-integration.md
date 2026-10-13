# 09 — HACS integration

- Type: task
- Blocked by: #8
- Status: open

## Goal

The Python HACS custom component installs from the repository, connects only to
the core's private HTTP+WS API, and projects sensing results into Home Assistant
entities, a config flow and a sidebar entry — presentation and transport only,
never sensing logic.

## Implements

- `docs/spec/roadmap.md` Slice 7 (HACS half).
- `docs/spec/home-assistant.md` §3.2 (component layout, manifest), §4 (naming),
  §5 (component ↔ core contract), §6 (entity model, coordinator, stream), §7
  (config flow), §8 (panel registration), §9 (install/upgrade).
- `docs/spec/core-api.md` (the private contract it consumes).
- `docs/spec/privacy-security.md` G5, G8 (ingress identity; no public surface).

## References

`hacs`, `ha-hacs` (publishing), `ha-arch`, `ha-addons`.

## Acceptance criteria

- **Install gate:** the component installs from the published repository and
  reads only the host; `requirements` stays empty (aiohttp from HA).
- **Boundary gate:** the component owns no DSP/inference/fusion/calibration and
  speaks only to the core (risks.md R-28); review-enforced.
- **Null-honesty gate:** a `null`/`Unknown` reading maps to HA `unavailable`/
  `unknown`, never a fabricated zero; presence confidence is not a separate
  entity in v1.
- **Version gate:** the component compares `api_version`/`min_component_version`
  against `/api/v1/health` and reports a mismatch clearly rather than silently.
- **Stale gate:** a WS drop marks entities unavailable and re-snapshots on
  reconnect; a `seq` gap forces a resync.

## Test-first first step

Write a failing test with a recorded `/api/v1/health` fixture asserting the
config flow validates the core URL and creates one entry keyed by `instance_id`.
