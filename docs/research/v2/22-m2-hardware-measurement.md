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
| `m2_two_node_hardware.py` | the cross-node phase residual, paired by transmitter address and 802.11 sequence number |
| `m2_metric_audit.py` | whether the simulated red band survives a corrected residual |
| `phase_ambiguity_probe.py` | the structure of the per-frame phase ambiguity |
| `phase_rotation_probe.py` | the per-frame phase rotation, at one node and across two |
| `layout_flip_probe.py` | the negation rate per CSI layout, used to test the merge and scale hypotheses |
| `sign_predictor_probe.py` | whether any reported field predicts the sign |
| `flip_pattern_probe.py` | whether the sign sequence is periodic or counter-driven |
| `amplitude_predictor_probe.py` | whether the amplitude carries the sign |
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

The pairing was made exact still further by the 802.11 **sequence number**.
The node now reports it (see §4), so two frames of the same transmission are
identified by the pair of the transmitter's address and the sequence number,
with an arrival-time bound kept as a guard because the sequence number is
twelve bits and wraps. The pairing is no longer an inference from timing at
all: every frame carried a sequence number, and no link produced an ambiguous
pair.

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

Every measured subcarrier of every measured link sits at zero or π: 639 of
639, 532 of 532 and 532 of 532 for the three node pairs, with a p95 of
3.1416 rad against a red-band threshold of 1.0 rad. The signs agree on 49 to
50 % of packets.

The measurement was then repeated with the exact pairing — the 802.11
sequence number, added to the wire after the reverse engineering of §4 — and
it gave the same answer with 15 % more pairs (1290, 1253 and 1594 against
1106, 990 and 1206): every subcarrier still at zero or π, the signs still
agreeing on 49 to 50 %. Every frame carried a sequence number, and no link
produced an ambiguous pair. **The verdict is not an artefact of the
pairing.**

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

### 3.1 The noise floor, and the experiment that is now ready

`scatterer_response.py` measures the group delay of every link twice — a
baseline and an after-window — and reports the change against the
uncertainty. The uncertainty is *measured*, not assumed: the frames are
temporally correlated, so the per-frame spread divided by the square root of
the frame count is far too optimistic. Pricing in the drift, by taking the
standard error of the window's block means, is what makes the result honest.
(The first version of this measurement used the optimistic uncertainty and
reported 4.5-sigma changes with nothing moving; the block-mean version puts
the median change at 1.3 sigma, which is what a null should look like.)

With the nodes static, over consecutive 50-second windows:

| | median change | 90th percentile |
| --- | --- | --- |
| strong node-to-node links | 1.8 cm | 2.9 cm |

A person walking into a link changes the path by tens of centimetres, so the
experiment has an order of magnitude of headroom. The module has an
interactive mode that takes a baseline, waits for the operator, and measures
again, and it reports the links that moved separately from the links that
serve as controls: the person is on one link, and the other nodes' links to
the same transmitter are the controls.

The same measurement survives the training-field change: with node 1 rebuilt
for the high-throughput field alone, the group delay and the reciprocity
checks were unchanged.

### 3.2 The amplitude path is a precise observable

`atsi_live.py` measures the same way for the receive-only ambient path: the
log of the received power over each frame's usable subcarriers, per node.
Over consecutive 40-second windows with nothing moving:

| node | mean log-amplitude | per-frame spread | window-to-window change |
| --- | --- | --- | --- |
| 1 | −0.392 to −0.398 | 0.075 | **0.03 dB** |
| 2 | −0.431 to −0.433 | 0.076 | **0.01 dB** |
| 3 | −0.418 to −0.421 | 0.058 | **0.01 dB** |

**The node-local power is stable to three hundredths of a decibel.** A person
entering a zone perturbs the received power by several decibels, so the
occupancy detector has a signal-to-noise of a hundred to one. ATSI needs no
phase, so the sign flip does not touch it.

This is the asymmetry that runs through the whole investigation, and it is
worth stating plainly: **on this hardware the amplitude is a precise,
stable observable and the absolute phase is not.** The amplitude is the
project's instrument. The phase is not.

## 4. Reverse-engineering the ESP32-S3's CSI path

Two questions were put to the binary: *why* does the vector get negated, and
is the negation resolvable? The first is now answered — it is intrinsic — and
the second is answered: it is not.

### 4.1 What REA could and could not do

REA was pointed at the ESP32-S3's WiFi driver blobs
(`components/esp_wifi/lib/esp32s3/libpp.a`, the closed PHY and MAC objects
the node links against) and could not analyse them, for three independent
reasons, each confirmed rather than assumed:

* `open_binary` rejects the objects with *"unsupported ELF architecture"* —
  REA's supported set is x86, x86-64, ARM, ARM64 and MIPS, and Xtensa is not
  in it. The check is in REA's own dispatcher, before any provider runs.
* `inspect-binary-layout` — the one provider that does not need a decompiler —
  reports *"Initial layout profile requires ELF64 little-endian objects"*.
  The ESP32-S3's objects are ELF32, so its ELF parser will not take them
  either.
* `rea doctor` reports no analysis engine at all (Ghidra, Hopper and IDA are
  all absent) and that the host itself is unsupported: *"REA supports macOS
  12+, Ubuntu 24.04+, Fedora 41+, Nobara 44+, 64-bit Arch Linux, CachyOS,
  Omarchy"*, and this is NixOS 26.11.

The `pwntools-elf` provider was prepared properly for it — a caller-supplied
Python 3.12 with pyelftools 0.33, pwntools 4.15.0 and Unicorn 2.1.2 — and the
ELF32 rejection stands regardless.

So the instruction-level work was done with the toolchain that matches the
target: `xtensa-esp32s3-elf-objdump` on the extracted objects
(`libpp.a:wdev.o`, `hal_mac.o`, `if_hwctrl.o`) and on the ESP32-S3 ROM ELF.

### 4.2 What the disassembly showed

* **`wifi_csi_info_t.rx_seq` is the 802.11 sequence number.** In
  `wdev_csi_rx_process` the field at body offset 82 is filled from the
  receive descriptor: a 16-bit word at descriptor offset 22, shifted right by
  four. The 802.11 sequence-control field is sixteen bits with the fragment
  number in the low four, so the shift extracts the twelve-bit sequence
  number. The ESP-IDF header confirms the layout the disassembly derived
  (`rx_ctrl` at 0, `mac` at 48, `dmac` at 54, `first_word_invalid` at 60,
  `buf` at 64, `len` at 68, `hdr` at 72, `payload` at 76, `payload_len` at 80,
  `rx_seq` at 82).

  This is the actionable outcome of the reverse engineering: the node already
  had the exact key for "which of this station's frames is this one", and the
  wire now carries it. Wire **minor 2** adds `rx_seq` at body offset 26,
  replacing the reserved field. Minor 1 carries only the transmitter's
  address, and the two minors have the same body length, so the codec reads
  the minor to tell them apart rather than inferring the layout from the
  length — reading a minor 1 node's reserved field as a sequence number would
  be a silent misreading.

* **The callback plumbing does not touch the CSI data.** `wdev_csi_rx_process`
  copies 48 bytes of receive metadata, the two MAC addresses, the buffer
  pointer and the length into the `wifi_csi_info_t`, and calls the node's
  callback. The samples themselves arrive already formed. The negation is
  therefore upstream of everything the node can see or set.

* **`wdev_csi_len_align`** (in the ROM) takes a ten-bit field from the
  descriptor at offset 28, adds three and rounds down to a multiple of four.
  That is where the ESP32-S3's 376, 380 and 384-byte CSI lengths come from.

* **`wdev_csi_hw_bug_check` is a zero-length stub** on the ESP32-S3, so the
  driver's CSI hardware-bug check is compiled out for this target.

* **`ic_csi_set_config`** is a thin wrapper: it calls `phy_chan_dump_cfg` and
  `phy_chan_filt_set` and writes two PHY registers. That is the *whole* of
  the driver's control surface over the CSI acquisition, and it is worth being
  precise about: the eight bytes of `wifi_csi_config_t` carry four functional
  choices — which training fields contribute, whether the high-throughput
  estimate is merged with the legacy one, whether the channel filter is on,
  and how the samples are scaled. Everything else is a dump or a reserved
  bit. There is no hidden control.

  The ESP-IDF header documents one of those fields — `ltf_merge_en`, *"enable
  to generate htlft data by averaging lltf and ht_ltf data when receiving HT
  packet"* — which suggested a hypothesis worth testing (§4.3).

* **There is no sign-mask negation anywhere in the ROM.** The whole 382 KB of
  ROM text was disassembled and searched: 575 `xor` instructions, none of
  them with 0x80, and no `movi` of a 0x80 sign mask. A whole-vector
  negation of the int8 samples would be exactly an exclusive-or with 0x80,
  so the flip is **not** a post-processing step applied to the CSI buffer
  after the estimate. It is in the estimator's arithmetic, below everything
  the driver can reach.

### 4.3 Every configuration the driver exposes, tested and eliminated

**The LLTF/HT-LTF merge.** If the merge averages two independently estimated
pilot responses and they disagree in sign, the merged result is corrupted.
The merge cannot touch the 64-subcarrier layout, which is the legacy training
field alone. Measured per layout on the live nodes, the whole-vector negation
rate is the same in both:

| layout | whole-vector negated | unchanged | mixed |
| --- | --- | --- | --- |
| 64 (legacy field alone) | 211–240 of 410–469 | 177–217 | 10–49 |
| 128 (legacy + high-throughput) | 67–94 of 162–230 | 75–118 | 4–15 |

**The manual CSI scale.** The node sets `manu_scale` with a shift of four,
because the automatic scale leaves the samples in about thirty int8 levels
and that costs phase precision. If the manual scale were the negation, the
automatic scale would fix it. Node 1 was re-provisioned with `csi_shift =
255`, which the firmware reads as *"use the hardware's automatic scale"*:

| scale | amplitude (core units) | whole-vector negated, layout 64 |
| --- | --- | --- |
| manual, shift 4 | 0.805 | 227 of 425 (53.4 %) |
| automatic | **0.101** | **211 of 410 (51.5 %)** |

The amplitude fell by almost exactly the eightfold the three lost shift bits
predict, and the negation rate did not move. The manual scale is not the
cause either, and it is worth keeping, because it is worth eight times the
amplitude resolution.

**The training field.** If the legacy field's estimate were the problem,
the high-throughput field's alone would be clean. Node 1 was rebuilt with
`lltf_en = false`, which makes the node report the high-throughput estimate
alone — and the node then produces only the 64-subcarrier layout, as
expected. The negation rate was **52 %**, unchanged.

So the negation is in the high-throughput estimate as well as in the legacy
one: it is in the estimator, not in either pilot's sign convention. The
channel filter is not a candidate either — the node already runs with
`channel_filter_en` off.

**Every choice the driver exposes has now been tested.** The merge, the
scale, the filter and the training field: none of them changes the negation.

### 4.4 Is the flip a pattern?

If the estimator's sign decision were driven by a counter or a state
machine rather than by noise, it would leave a pattern. It does not.

* **No periodicity.** The autocorrelation of the sign sequence was measured
  at lags 1 to 64: the largest was +0.089 to +0.111 against a null of
  +0.20 for the largest of 64 lags at that sample size. Nothing stands out.
* **No dependence on any counter.** The sign was tested against the low bits
  of the node's capture sequence number, of the node's own microsecond clock
  and of the 802.11 sequence number, at widths from one bit to four: no
  dependence exceeded the null, where the null accounts for the number of
  groups and their sizes. (A bare threshold would have reported a
  dependence: with sixteen groups of fifty samples, the *expected* largest
  group's share under a fair coin is already 0.63.)
* **The amplitude says nothing.** The sign does not correlate with the
  frame's mean amplitude, with the amplitude's change since the previous
  frame, with the amplitude profile's slope, or with the received power —
  every correlation is at or below 0.065. The frames that were negated and
  the frames that were not have the same mean amplitude to three decimal
  places (0.8031 against 0.8038, 0.8135 against 0.8123, 0.7842 against
  0.7887).

**The sign is noise in the estimator.** Nothing the node can report carries
it, so no correction is possible from the data.

### 4.5 What follows

Every hypothesis survives no: the merge, the scale, the filter, the training
field, a counter-driven pattern and the amplitude. The callback plumbing does
not touch the samples, and the ROM has no sign-mask negation to find. The
negation is intrinsic to the ESP32-S3's channel estimate: a property of the
receiver, applied per frame, independent at each receiver, and with no
reference in the data that could resolve it. That is the same verdict §2
reaches from the measurement, and the reverse engineering is what rules out
"it is something we configured".

**What "driving the hardware manually" can and cannot buy.** The driver's
control surface over the CSI acquisition has been enumerated in full — four
functional choices, three PHY register writes and two PHY calls — and every
one of them has been tested and none of them changes the negation. There is
no further lever above the MAC. Below it, the CSI samples are produced by
the RF path, which is closed and unnamed in the ROM; the 382 KB of ROM text
contains no sign-mask negation, so the sign is decided inside the
estimator's arithmetic rather than by a step that could be found and undone.
So the absolute cross-node phase is not recoverable on this hardware by any
route available here.

**What is left is the part that does not need the sign.** A whole-vector
negation shifts every subcarrier's phase by the same pi, so it leaves the
phase *differences* between subcarriers untouched — and the phase slope, the
per-link group delay, is exactly invariant to it. The measurement of §3 is
that quantity, and it is validated. It also survives the training-field
change: with node 1 rebuilt for the high-throughput field alone, the group
delay and the reciprocity checks were unchanged (1.3 cm, 5.8 cm, 3.2 cm and
1.9 cm apart). The project does not need the absolute phase; it needs the
path length, and the path length is measurable.

## 5. What this means for the project

1. **Everything gated on M2 is gated.** Closure-phase tomography (BCPT), RCIA
   and NSDT are off the table on ESP32-S3 raw CSI, and now on a measurement
   rather than on an assumption. The ranking in `21-3d-imaging-concepts.md`
   should be read with M2 measured red, not unmeasured.
2. **The amplitude path is unaffected, and it is precise.** FZOG and ATSI do
   not use phase. The node-local power is stable to 0.03 dB across
   consecutive windows, so an occupancy detector has a signal-to-noise of a
   hundred to one against a person entering a zone.
3. **A phase-derived path is still available.** The group delay is
   gauge-invariant, needs no common reference, and is validated to a couple of
   centimetres by reciprocity. Its noise floor is 2 to 3 cm per window, and
   the experiment that asks whether it responds to a person moving between the
   nodes is built and ready (`scatterer_response.py`).
4. **The simulated M2 was never the gate.** The hardware measurement is now
   wired into the campaign, so the ledger's M2 entries record the hardware
   source and the hardware number. The ATSI phase is on the hardware too.
   FZOG remains simulated, because its calibration needs a person standing
   in each of the nine voxel positions and the nodes' physical positions are
   not yet known to the project — that is a protocol, not a code change, and
   it is the next thing to run with a person in the room.

## 6. Caveats

* The nodes were all in the same room on the same channel, with the other
  nodes' own CSI traffic as the dominant ambient signal. A different traffic
  mix would change the pair counts, not the ambiguity.
* The measurement covers three nodes on one channel. Nothing here says
  anything about a wider channel plan.
* The sign ambiguity is a property of the ESP32-S3's reporting. §4 rules out
  the two configurations the node controls — the LLTF/HT-LTF merge and the
  manual CSI scale — and the callback plumbing does not touch the samples, so
  the negation is intrinsic to the receiver's channel estimate. Exactly where
  in the closed PHY it comes from was not established.
* The group-delay figure is a path *difference* from the phase slope, not a
  range. It is a relative quantity and it needs a common reference to become
  an absolute position.
