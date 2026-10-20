# Next session: the prompt

A self-contained prompt for a fresh agent session, to be pasted as the whole
first message. It is written to be worked through without help from the
operator, and it names the landmines so they are not rediscovered the hard
way.

---

## The prompt

You are continuing work on **Aquila**, a WiFi CSI sensing project at
`/home/matt/projects/aquila-v2-spec`. Work autonomously and do not stop to ask
questions; if a decision is needed, make it, record the reasoning in the repo,
and keep going. Prose uses Australian English. This is research work: patent
material is draft only and nothing here is legal advice.

**Read first, in this order:**

1. `AGENTS.md` (builds, shells, doc layout — there is no global `cargo`)
2. `CONTEXT.md` (the domain glossary; use its vocabulary exactly)
3. `docs/research/v2/22-m2-hardware-measurement.md` — **the state of the
   science.** This is the single most important document: it records the
   measurement that closed the cross-node-phase question, and what remains
   open.
4. `docs/research/v2/21-3d-imaging-concepts.md` — the approach ranking, and
   which approaches the M2 result gates.
5. `docs/research/v2/experiments/README.md` and `EXPERIMENT_PLAN.md`

**Where the project stands.** Three ESP32-S3 nodes stream real CSI to the
core at 10.1.1.3:9100 over UDP, on channel 5, at about 20 frames per second
each, and each node hears the other two. The nodes take signed firmware
updates over the air. The decisive measurement is done: **the ESP32-S3's CSI
phase is not coherent across nodes.** Each frame's complex channel is reported
with an arbitrary sign — a whole-vector negation — and the negation is an
independent coin flip at each receiver, so the cross-node phase of one packet
is ambiguous modulo pi with no resolvable reference. Every measured
subcarrier sits at zero or pi, and the M2 gate is red on hardware. The
simulated red band that used to be recorded was a metric artefact (it measured
the arbitrary static path phase instead of the phase's stability) and has been
fixed.

**What still works, and is the direction to take.** The phase *slope* across
subcarriers is exactly invariant to the sign flip, and it is the per-link
group delay. It is measured on the live nodes and validated: two nodes
independently measuring each other agree on the path length to about two
centimetres, with a per-frame spread of about a decimetre. This is the
quantity to build on. It is gauge-invariant, needs no common reference, and no
cross-node phase coherence.

**Your task, in priority order.**

1. **Measure whether the group delay responds to a person moving between the
   nodes.** This is the next experiment and it is the one the project now
   turns on. Take a baseline of the per-link phase slope over a couple of
   minutes, then take another while a person walks the length of the link
   between two nodes, and report the change in the slope-implied path
   difference against the noise floor. `group_delay_hardware.py` already does
   the measurement; it needs a before/after comparison and a sensitivity
   number. Write the result up the way `22-m2-hardware-measurement.md` does.
   If the person is not available, use a proxy that moves the channel
   (a swung door, a chair, a person in the room) and say plainly in the write
   up what the proxy was.
2. **Extend the delay measurement to a moving-scatterer model.** The group
   delay of a link is the weighted mean path delay of its multipath
   components; a moving person adds a component whose delay changes. Write
   down what the measured slope change implies about the scatterer's range,
   and check it against the geometry. The concepts document's approach 7
   (per-link group delay) and approach 4 (phase slope `dφ/df`) are the
   relevant ones; neither is gated on M2.
3. **Make the campaign runner use the hardware measurement end to end.** The
   M2 phase already does. The FZOG and ATSI phases still run in simulation;
   give them the same treatment — measure on the live nodes when nodes are
   streaming, fall back to the model when they are not, and record which was
   used in the ledger, as `record_m2_result` now does.
4. **Recover node 4.** Node 4 (`ac:a7:04:e2:0a:74`) is still on the firmware
   built before the provisioning fix and cannot associate. It needs one USB
   reflash with the current build and the node's NVS image; the other three
   were recovered this way and then updated over the air. With four nodes the
   measurement has a second geometry, and the reciprocity check becomes a
   triangle rather than a pair.

**Landmines — these cost hours if rediscovered.**

* **Opening the serial port resets the ESP32.** `idf.py monitor`, `esptool` and
  anything else that opens `/dev/ttyACM*` will reset the board and abort an
  in-flight OTA. Never open it while an OTA is running.
* **A node's control socket binds an ephemeral source port.** The core must
  learn that port from the node's own datagrams. Never assume 9101 is the
  port to reply to; 9101 is the port the *core* listens on.
* **`ota-push` must keep holding the control channel open through the health
  window.** The node's `OTA_RESULT` with outcome `Verified` arrives *before*
  the reboot; `Activated` or `Rollback` arrives after the health window. A
  tool that stops on `Verified` makes the node fail its own health check and
  roll the image back. This was the cause of "successful" updates that
  silently reverted.
* **`--token-file` and `--pairing-code` conflict.** A node whose NVS was wiped
  by a USB reflash is unpaired and sends `JOIN_REQUEST`; it must be paired
  with `--pairing-code`, and `--token-file` is then where the new token is
  written. Passing `--token-file` without `--pairing-code` means "reuse the
  stored token" and will not pair an unpaired node.
* **NVS keys are at most fifteen characters.** A longer key fails silently, so
  a key like `security_version` reads back as absent. `sec_version` and
  `ctrl_port` are the names in use.
* **`cmath.phase()` on a real float returns 0 or pi, not the argument.** It is
  a trap that has already produced three wrong measurements in this project
  (a "settled versus flipped" histogram that was really just the sign of the
  difference, and a "concentration 1.0" that was an artefact of unwrapping an
  alternating series into a ramp). Wrap with `cmath.phase(x - y)` or subtract
  explicitly; never `cmath.phase(real_difference)`.
* **CSI frames come in two layouts, 64 and 128 subcarriers, and index *k* is a
  different physical subcarrier in the two.** Never compare a phase across
  layouts. Within the 128 layout, fit the slope over the legacy long training
  field alone (indices 0 to 63); the high-throughput half has a different tone
  arrangement. The firmware's reported `subcarrier_offset` is a provisional
  `-(n/2)` and does not describe the physical mapping.
* **Subcarriers 0 and 28 to 33 are exactly zero.** A null subcarrier's phase
  is a constant, which flatters any residual. Skip them.
* **The null block the ESP32-S3 leaves mid-payload measures a degenerate
  exactly-zero residual.** Do not treat it as a good subcarrier.
* **`nix develop` rewrites `flake.lock`.** Restore it before committing.
* **Seven `aquila-testkit` tests fail on a clean tree** (verified on
  `1cd5313` in a separate worktree: `coarse_pose_meets_the_torso_pck20_bar`,
  `floor_measurements_are_recorded`,
  `floor_presence_and_count_meet_the_slice5_bars`, `pose_rows_are_recorded`,
  `pose_search_band_is_not_keyed_to_the_simulator`,
  `home_measurements_are_recorded`,
  `whole_home_presence_count_and_attribution_meet_the_slice6_bars`). Do not
  chase them.
* **`data/ota/fw_ed25519.seed` is the firmware signing key.** It is gitignored
  and must never be committed or rotated; the public key is in every node's
  NVS.
* **Do not `git commit` the build directory or the `data/ledger` scratch
  unless the record is the point.** The ledger entries are evidence; add to
  them through `record_*_result` and `append_ledger_entry`.

**How to build and test.**

```
nix develop --command cargo test --workspace     # 7 known testkit failures
nix shell nixpkgs#cmake nixpkgs#ninja nixpkgs#gcc -c bash -c '
  export LD_LIBRARY_PATH=/nix/store/rv7zdild7njfakhcvl7jgcly91x8s91a-libusb-1.0.30/lib:$LD_LIBRARY_PATH
  source ~/.espressif/python_env/idf5.5_py3.14_env/bin/activate
  . ~/esp/esp-idf/export.sh
  cd firmware/esp32-s3-csi-node && idf.py build'
```

The experiment scripts run with plain `python3` from
`docs/research/v2/experiments/`.

**Deliverables.** A write-up in `docs/research/v2/` in the style of
`22-m2-hardware-measurement.md`, with the raw measurement records under
`docs/research/v2/experiments/data/`; the code that produced them; and a
commit. Update `21-3d-imaging-concepts.md` and `00-synthesis.md` if the result
changes what is gated on what. If a measurement comes out negative, write it
up as a negative result with the evidence — that is a deliverable here, not a
failure.

---
