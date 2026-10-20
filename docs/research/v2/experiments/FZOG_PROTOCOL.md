# The FZOG hardware protocol

FZOG is the campaign's primary result — the amplitude-first occupancy
imaging — and it is the one phase that still runs in simulation. It stays
simulated for a reason that is not a code problem: **the calibration needs a
person standing in each voxel position**, and the campaign has never had one.

`fzog_live.py` is the hardware path, ready to run. This is how to run it.

## What it does

A hardware *link* is a *(transmitting station, receiving node, subcarrier)*.
The transmitting station is named by its MAC address, so the calibration and
the scoring see the same links. In practice the other nodes dominate — they
stream their own CSI to the core — and the access point is the rest.

The calibration is a dictionary from link to a list of log-amplitudes, one
per voxel, in the voxel order. That is exactly the shape the simulation's
`score_voxel` consumes, so the same scoring code serves both.

## Before you start

1. **All three nodes streaming**, on the same channel, at about 20 frames a
   second each. `python3 hardware.py`'s collector, or the campaign runner,
   will show them.
2. **A voxel grid** over the region of interest. Copy
   `VOXELS.example.json` and edit the labels and positions. The positions
   are for the human — the scoring only uses the order and the labels.
3. **A person** who can stand in each voxel on request.

## Calibrating

```
python3 fzog_live.py --voxels my_voxels.json --calibration my_calibration.json --calibrate
```

The tool walks the voxel list. For each one it prints the label and waits:
put the person there and press enter, or press `s` to skip the voxel. A
skipped voxel is left uncalibrated and is not scored later.

Each voxel's window is 20 seconds by default (`--duration`). That is long
enough for the link amplitudes to settle; the tool reports how many link
amplitudes it recorded and how many frames they came from, so a window that
caught a bad patch of traffic is visible.

The calibration is written to the file at the end, and also whenever the
walk stops early.

**The walk is worth doing twice.** The per-link amplitude floor is about
0.036 in the log-amplitude domain, so a single calibration column carries a
few per-cent uncertainty. Two walks, one per person placement, halve it. Run
the second walk to a separate file and average the two columns.

## Scoring

```
python3 fzog_live.py --voxels my_voxels.json --calibration my_calibration.json
```

The tool records the links twice, reports the noise floor it measured, and
scores every calibrated voxel. It prints the best voxel, the margin over the
runner-up, and the score's own uncertainty, and it says plainly whether the
margin is inside the noise.

That last line is the one to read. A margin below the uncertainty is not a
detection, and the tool says so rather than reporting a best voxel.

## Reading the noise

Two floors are reported, and the difference between them matters:

* the **per-subcarrier** floor — the largest change in any single
  subcarrier's amplitude between two windows, about 0.22;
* the **per-link** floor — the largest change in a link's mean amplitude,
  about 0.036.

The per-subcarrier floor is much the larger, because a fading event moves
every subcarrier of a link together. The score is a mean over links, so its
uncertainty is the **per-link** floor over the square root of the number of
links — about 0.002 with five hundred links. Using the per-subcarrier floor
here would be wrong by an order of magnitude, and would report every
detection as noise.

## What to expect

A person standing in a voxel changes the amplitude of the links whose path
they disturb. The group's own link model puts the effect at a few per cent
of the link amplitude for a person close to the link
(`scatterer_sensitivity.py` gives the equivalent figure for the group
delay). With a few hundred links, that is well above the score's
uncertainty.

The hard case is a person standing exactly on the line of sight, where the
direct path is shadowed and the change partially cancels in the weighted
mean. Nothing in the method detects that reliably, and the report will show
a margin inside the noise rather than a false positive.
