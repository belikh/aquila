# M2 on real hardware: the ESP32-S3's phase is not coherent across nodes

**Status: measured. The M2 cross-node phase residual is RED on ESP32-S3, and
the reason is structural rather than incidental. A gauge-invariant
alternative — the per-link group delay — is measured and validated.**

This note supersedes the standing assumption that M2 is "unmeasured on
ESP32-S3 wireless-only" (`21-3d-imaging-concepts.md` §M2) and the earlier
reading that the simulated red band was a finding about the channel. It
records what was measured, on three live nodes, and what it means for the
project's direction.

Every number here comes from the real ESP32-S3 nodes streaming to the core.
The measurement scripts are in `docs/research/v2/experiments/`:

| script | what it measures |
| --- | --- |
| `m2_two_node_hardware.py` | the cross-node phase residual, paired by transmitter address |
| `m2_metric_audit.py` | whether the simulated red band survives a corrected residual |
| `phase_ambiguity_probe.py` | the structure of the per-frame phase ambiguity |
| `phase_rotation_probe.py` | the per-frame phase rotation, at one node and across two |
| `group_delay_hardware.py` | the per-link group delay from the phase slope |
| `sign_resolution_probe.py` | whether the sign flip can be resolved from the data |

The raw records are produced under `docs/research/v2/experiments/data/m2/`. That
directory is a local output — the repository ignores
`docs/research/v2/experiments/data/` — so the records are not committed; the
numbers quoted below are the ones they contain.

## 1. What had to be built first

### 1.1 The nodes could not say who sent a packet

M2 compares two nodes' phases for *the same transmission*. Until the node
reported the transmitting station's address, the core could only guess which
captured frames were the same packet, and the guesses were wrong in a way
that produced a uniformly random residual rather than a large one:

* the access point's beacons from two access points interleave at about half
  the beacon interval, so the "periodic" part of the stream mixes senders;
* every beacon of one access point arrives at nearly the same power, so
  received power does not discriminate either.

Pairing on arrival time and received power gave a matched-pair phase
concentration of 0.029 against a control of 0.040 — indistinguishable from
noise, which is what a wrong pairing looks like.

The ESP32-S3's driver already reports the transmitting station's address
(`wifi_csi_info_t.mac[6]`), so the fix was to carry it. Wire **minor 1** adds a
six-byte `source_mac` to the AQCI body; the Rust codec and the generated
`node_wire.h` were changed together and the firmware copies the driver's
address verbatim. With it, the pairing is exact: 546 packets paired for one
node pair with **0 ambiguous**.

The change is documented in `docs/spec/capture-and-firmware.md` §5.1 and
`docs/spec/data-contracts.md` §9.1. Wire minor 0 is still read, so a node that
has not been updated reports frames without a transmitter.

### 1.2 Two tool faults that made every update roll back

Both were found only by checking what the nodes actually reported afterwards,
not by reading the tool's output:

* **`ota-push` stopped holding the control channel on the first outcome.** The
  node's `OTA_RESULT` with outcome `Verified` is sent *before* the reboot; the
  outcome that matters (`Activated`, or `Rollback`) comes after the health
  window. Returning on `Verified` meant the core stopped pinging the node, the
  node's own health probe saw an unresponsive control channel, and the node
  rolled the image back. Every update "succeeded" and every node reverted.
* **`--token-file` was both the read source and the write target.** Passing it
  forced the already-paired path, so the pairing path was unreachable and a
  node whose NVS had been wiped by a USB reflash could never be paired again.

Both are fixed. Three nodes now take an air update, pair, pass the health
window and report `Activated`.

## 2. The measurement, and what it found

### 2.1 The phase is physical within a frame

Within one frame the phase across the subcarriers is smooth and physical: on
node 1 receiving node 2's transmissions, the phase runs from −2.17 rad at
subcarrier 1 to −1.93 rad at subcarrier 19 and −2.48 rad at subcarrier 48. The
amplitude is tight (0.61 to 0.97 in the core's unit). The ESP32-S3 also leaves
a null band — subcarriers 28 to 33 are exactly zero, and subcarrier 0 is the
centre carrier — which any phase measurement has to skip.

### 2.2 The whole vector is negated from frame to frame

Between consecutive frames the phase difference between two subcarriers is
unchanged (0.377 to 0.526 rad between subcarriers 10 and 50 across every
frame), while the absolute phase moves freely. The sign flip is **coherent
across the whole vector**: between consecutive frames of one transmitter, 191
frame pairs had every subcarrier's sign preserved and 205 had every
subcarrier's sign flipped, with almost nothing in between.

So a frame's complex channel is reported with an arbitrary sign: the absolute
phase is known only modulo π.

### 2.3 The two nodes do not agree on the sign

For the *same* packet at two nodes, the whole-vector sign agrees on 74 pairs,
disagrees on 84 and is mixed on 25 (183 pairs). The cross-node phase
difference is therefore bimodal at zero and π, with the signs agreeing on
**49 to 50 %** of pairs: a coin flip, independent at each receiver.

There is no reference that could resolve it. Resolving the sign at one node by
temporal continuity is possible in the sense that every consecutive transition
then correlates positively — but that is circular, because the true
correlation between consecutive frames of one transmitter is itself
|correlation| ≈ 0.63 with a random sign, which is exactly the statistics of a
uniformly random phase reference.

The decisive comparison: the correlation between two nodes' views of the *same*
packet has the same distribution as the correlation between *consecutive*
frames at one node (mean |correlation| 0.65 against 0.63; both equal
2/π ≈ 0.637, the mean of |cos| for a uniform phase difference). **The two
nodes' absolute phases for one packet are no more related than two unrelated
frames are.**

### 2.4 The verdict

Every measured subcarrier of every measured link sits at zero or π: 529 of
529, 532 of 532 and 581 of 581 for the three node pairs, with a p95 of
3.1416 rad against a red-band threshold of 1.0 rad.

**M2 is red on ESP32-S3. Cross-node coherent absolute phase — closure-phase
tomography, BCPT, RCIA — is not available from this hardware's raw CSI.** The
blocking artefact is a discrete per-frame sign that flips independently at
each receiver, not noise, drift or a pairing error, and no post-processing
removes it.

This is a real negative result and it settles the question the research
document listed as the go/no-go for every coherent approach
(`21-3d-imaging-concepts.md`, M2).

### 2.5 The simulated red band was a metric artefact

The campaign's earlier "M2 red → BCPT hardware-gated" came from the
simulation. `m2_metric_audit.py` shows why, by computing the residual two ways
from the same simulated phases:

| link | as implemented | centred | intercept |
| --- | --- | --- | --- |
| TX0→RX0 | 2.7762 (red) | 0.0891 (green) | −2.7139 |
| TX0→RX1 | 2.2240 (red) | 0.0850 (green) | +2.1445 |
| TX0→RX2 | 3.0884 (red) | 0.0994 (green) | −3.0090 |
| TX0→RX3 | 1.3984 (red) | 0.1009 (green) | +1.3172 |
| TX0→RX4 | 0.4843 (amber) | 0.0900 (green) | +0.4098 |
| TX0→RX5 | 3.0819 (red) | 0.1054 (green) | +3.0083 |

The as-implemented residual subtracts the drift but not the constant, and the
constant is the static path phase — whose value depends on the path length
modulo a wavelength, so it is arbitrary and differs for every link. The
as-implemented residual therefore measures where the path length happens to
fall rather than how well the phase is held. Removing it turns every link
green, with a p95 of about 0.09 rad, which is what the simulation's 0.05 rad
phase noise predicts.

The metric is fixed in `m2_characterization.py`, and the campaign's M2 phase
now measures the **hardware** when nodes are streaming, falling back to the
simulation only when they are not.

(A separate artefact in the same simulation: the modelled 100 Hz carrier
offset aliases exactly at the simulation's 100 Hz sample rate, so the fitted
drift is 0.0 rad/s. It does not affect the residual.)

## 3. What still works: the per-link group delay

A whole-vector sign flip shifts every subcarrier's phase by the same π, so it
leaves the phase *differences* between subcarriers untouched. The slope of the
phase against subcarrier index is therefore exactly invariant to the
ambiguity, and it is the per-link group delay:

    delay = slope / (2π × 312.5 kHz)

`group_delay_hardware.py` measures it on the live nodes. The slope is fitted
over the legacy long training field only, because that part is the same set of
physical subcarriers in both layouts the node reports (64 subcarriers, or 128
with the high-throughput field appended), so a slope measured on one means the
same thing as a slope measured on the other.

**The reciprocity check is the validation.** Node A's link to B and node B's
link to A are two independent measurements of the same geometry, so they must
agree:

| pair | node A's path | node B's path | difference |
| --- | --- | --- | --- |
| node 1 ↔ node 2 | −0.903 m | −0.904 m | **0.0 cm** |
| node 1 ↔ node 3 | −1.196 m | −1.187 m | **0.9 cm** |
| node 2 ↔ node 3 | −0.776 m | −0.755 m | **2.1 cm** |

(The 64-subcarrier layout of the 1↔3 pair is the one exception, at 7.1 cm;
it carries a handful of frames with a corrupted slope, and its per-frame
spread is an order of magnitude larger than the others'.)

The per-frame spread of the slope is about 0.0006 rad per subcarrier, which is
a path-length uncertainty of about 9 cm. The 128-subcarrier and 64-subcarrier
layouts agree to about 2 cm on the same link.

**So the ESP32-S3 can measure the per-link path length to about a decimetre,
without any cross-node phase coherence at all.** That is exactly the
quantity the gauge-invariant approaches in `21-3d-imaging-concepts.md` need —
approach 4 (the phase slope `dφ/df`, which needs no common transmitter and no
reciprocity) and approach 7 (per-link group delay). Neither is gated on M2.

## 4. What this means for the project

1. **Everything gated on M2 is gated.** Closure-phase tomography (BCPT), RCIA
   and NSDT are off the table on ESP32-S3 raw CSI, and now on a measurement
   rather than on an assumption. The ranking in `21-3d-imaging-concepts.md`
   should be read with M2 measured red, not unmeasured.
2. **The amplitude path is unaffected.** FZOG and ATSI do not use phase.
3. **A phase-derived path is still available.** The group delay is
   gauge-invariant, needs no common reference, and is validated to a couple of
   centimetres by reciprocity. It is the phase-derived quantity to build on,
   and it is the subject of the next measurement: whether it responds to a
   person moving between the nodes, and with what sensitivity.
4. **The simulated M2 was never the gate.** The hardware measurement is now
   wired into the campaign, so the ledger's M2 entries record the hardware
   source and the hardware number.

## 5. Caveats

* The nodes were all in the same room on the same channel, with the other
  nodes' own CSI traffic as the dominant ambient signal. A different traffic
  mix would change the pair counts, not the ambiguity.
* The measurement covers three nodes on one channel. Nothing here says
  anything about a wider channel plan.
* The sign ambiguity is a property of the ESP32-S3's reporting. Whether it is
  the receiver's phase reference, a hardware sign flip, or a consequence of
  the manual CSI scale (`csi_shift = 4`) was not separated; the sweep in
  `scripts/sweep-csi-shift.sh` would test the last of those.
* The group-delay figure is a path *difference* from the phase slope, not a
  range. It is a relative quantity and it needs a common reference to become
  an absolute position.
