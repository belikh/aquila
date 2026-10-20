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

1. **Run the moving-scatterer experiment.** It is built and ready:
   `docs/research/v2/experiments/scatterer_response.py --interactive`. It
   takes a baseline of the per-link group delay, waits for the operator, then
   measures again, and reports the links that moved separately from the
   links that serve as controls. The noise floor is measured at 2 to 3 cm per
   window, and the project's own link model predicts that a person within a
   metre of the link moves the group delay by two to ten times that floor
   (`scatterer_sensitivity.py`). **This needs a person in the room** — it is
   the one thing the project cannot finish alone.
2. **Run the occupancy check the same way.**
   `docs/research/v2/experiments/atsi_live.py --interactive`. The node-local
   power is stable to 0.03 dB, so a person entering a zone should be obvious.
3. **The FZOG calibration protocol.** FZOG is the last simulated phase, and
   it is simulated because its calibration needs a person standing in each of
   the nine voxel positions *and* the nodes' physical positions, which the
   project does not know. Get the node positions from the operator, write the
   protocol down, and then run it: for each voxel, a person stands there
   while the nodes record the link amplitudes, and the classifier scores the
   occupancy. This is the one remaining claim in the campaign that is not on
   the hardware.
4. **Recover node 4.** Node 4 (`ac:a7:04:e2:0a:74`) is still on the firmware
   built before the provisioning fix and cannot associate. It needs one USB
   reflash with the current build and the node's NVS image; the other three
   were recovered this way and then updated over the air. With four nodes the
   reciprocity check becomes a triangle rather than a pair.

**Landmines — these cost hours if rediscovered.**

* **REA cannot analyse the ESP32-S3's binary blobs.** Three independent
  reasons, each confirmed: its dispatcher rejects Xtensa ("unsupported ELF
  architecture" — its supported set is x86, x86-64, ARM, ARM64, MIPS); its
  ELF layout provider requires ELF64 and the ESP32-S3's objects are ELF32;
  and `rea doctor` reports no analysis engine installed (Ghidra, Hopper and
  IDA are all absent) and that this host (NixOS 26.11) is itself outside
  REA's supported list. Use `xtensa-esp32s3-elf-objdump` on the objects
  extracted from `components/esp_wifi/lib/esp32s3/libpp.a` instead — that is
  what produced §4 of the research note.
* **The ±π sign flip is intrinsic to the ESP32-S3.** Every configuration the
  driver exposes has been tested and eliminated: the `ltf_merge_en` merge
  (the negation rate is the same in the 64-subcarrier legacy-field-only
  layout, which the merge cannot touch), the manual CSI scale (the hardware's
  own automatic scale negates just as often — and costs eight times the
  amplitude resolution, so keep `csi_shift = 4`), the channel filter (it is
  already off), and the training field (a rebuild with `lltf_en` disabled
  still negates 52 % of the time). The callback plumbing does not touch the
  samples, and the ROM has no sign-mask xor to find. The flip is not a
  counter-driven pattern and the amplitude says nothing about it. Do not
  re-open these.
* **The uncertainty of a window's mean is not the per-frame spread divided
  by the square root of the frame count.** The frames are temporally
  correlated, so that is far too optimistic — it reported 4.5-sigma changes
  with nothing moving. Use the scatter of the window's *block* means, which
  is what `group_delay_hardware.py` and `atsi_live.py` now do.
* **The 802.11 sequence number is on the wire now** (wire minor 2, body
  offset 26). The node reports it for 100 % of frames and it increments
  visibly, so the cross-node pairing is exact — pair on
  `(source_mac, rx_seq)` and keep the arrival-time bound only as a guard
  against the twelve-bit wrap.
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
