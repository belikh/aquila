# 21 — 3D Indoor Imaging Concepts for ESP32-S3 CSI

**Project:** Aquila — WiFi CSI sensing on ESP32-S3 nodes
**Status:** Concept synthesis (new imaging approaches, not inventions a–t or u–w)
**Spelling:** Australian English
**Prepared:** 2026-10-18

## Purpose

This note proposes **nine distinct 3D indoor imaging approaches** that could be built on the ESP32-S3 CSI pipeline and could sit alongside (or extend) existing inventions a–t and u–w. Every approach is grounded in the **observability boundary** formalised in note 20 and synthesised in note 00:

- With independent LOs, per-link CSI on subcarrier `k` is

  ```
  ĥ_ij[k] = g_ij[k] · a_i · b_j · e^{j(θ_i − ψ_j)} · e^{−j2π f_k τ_ij}
  ```

  and **only `θ_i − ψ_j` is observable** — a per-component `U(1)` phase (plus time/frequency) gauge.
- **Gauge-invariant (usable, no shared reference):** amplitude `|ĥ|`; per-link subcarrier differential phase → delay/ToF `τ_ij`; inter-antenna differential phase within one receiver → AoA; two-way products `ĥ_AB · ĥ_BA`; temporal differentials → Doppler.
- **Provably unobservable without an added reference:** absolute cross-node phase; absolute range-from-carrier-phase; absolute synchronisation; cross-receiver AoA.
- **Minimal added reference (one per connected component):** (1) common-transmitter broadcast; (2) one OTA reciprocal round trip per link `ĥ_AB · ĥ_BA⁻¹`; (3) shared/conducted clock + tone.
- **M2:** per-packet cross-node phase residual after CFO removal — **unmeasured on ESP32-S3 wireless-only**. Every coherent approach below is gated on M2; every incoherent approach must work if M2 fails.
- **ESP32-S3:** int8 CSI, 20/40 MHz (~64/128 subcarriers), 2.4 GHz only, nanosecond RX timestamp (12.5 ns grid, ~20.8 µs latch), **no TX hardware timestamp**; CSI ping = ESP-NOW HT40 MCS0; range resolution `c/2B` ≈ 7.5 m (20 MHz) / 3.75 m (40 MHz) raw bin — sub-metre imaging requires synthetic bandwidth or delay-gating, not a single 20 MHz channel.

None of these repeats inventions a–t or u–w verbatim. They are imaging approaches; several use closure phase or reciprocity only as a *component*, not as the whole invention.

---

## Approach 1 — Multi-Static Per-Link Delay Tomography (MPDT)

**1. Approach name:** Multi-Static Per-Link Delay Tomography (MPDT)

**2. What it images:** Both static structure and people (volumetric). Static walls/furniture give stable per-link delays; a person's body adds a delayed scattered path on links crossing them.

**3. Core mechanism / equations:** Round-robin: each node transmits an ESP-NOW HT40 MCS0 CSI ping; all others receive. For each ordered link `i → j`, fit the measured phase vs subcarrier frequency to a line:

```
φ_ij[k] = ∠ĥ_ij[k] = C_ij − 2π f_k τ_ij            (C_ij = θ_i − ψ_j, drops out of slope)
τ_ij = −(1/2π) · dφ_ij/df                         (group delay, gauge-invariant)
ρ_ij = |ĥ_ij|                                      (amplitude, gauge-invariant)
```

Build the full `N × N` delay matrix `D` and amplitude matrix `A` (N = 10). A scatterer at 3D position `r` contributes a bistatic delay on link `(i, j)`:

```
τ_ij(r) = (|r − p_i| + |r − p_j|) / c            (reflection path)
```

or `|r − p_j| / c` for the direct path. Invert for 3D reflectivity `σ(r)` by travel-time tomography:

```
σ̂(r) = argmin_r Σ_{i,j} w_ij [ τ_ij^meas − τ_ij(r) ]² ,    w_ij ∝ ρ_ij
```

solved by backprojection / Eikonal inversion. Vertical spread of perimeter nodes gives elevation (`z`); multi-channel stitching (Approach 7) supplies the bandwidth for range resolution.

**4. Observability analysis:** Fully gauge-invariant — uses only per-link `|ĥ|` and the per-link phase **slope** `dφ/df`, both of which cancel the unknown `θ_i − ψ_j`. Needs **no cross-node phase coherence**, no common transmitter, no reciprocity. **M2 irrelevant:** works even if M2 fails. The only caveat is delay ambiguity `τ mod (1/B)`; at 20 MHz that is a 50 ns (≈15 m) wrap, acceptable within a home but multipath can alias — hence stitch channels (Approach 7) or gate (Approach 5).

**5. Prior-art position:** WiFi travel-time tomography exists (e.g. WTA-class systems) but generally assumes wideband coherent phase or dense AP deployment. Consensus found no ESP32-S3 narrowband per-link delay tomography with channel stitching. **White space:** gauge-invariant narrowband delay tomography on commodity 2.4 GHz ESP32-S3, deliberately avoiding any cross-node phase assumption.

**6. FTO risk:** Low–moderate. Aerial's occupancy family (US12044789B2) covers per-zone occupancy via multiple transceivers — different from tomographic delay inversion. Closure phase is unused here (open niche, not infringed). No 11bf SEP exposure (legacy CSI capture only).

**7. ESP32-S3 feasibility:** RX-only receivers; one node TX at a time (round-robin). 2.4 GHz only. Single 20 MHz channel gives a 7.5 m bin — too coarse for sharp 3D — so MPDT *must* pair with channel stitching (Approach 7) or accept room-scale/people-scale only. ESP-NOW HT40 MCS0 CSI ping is the native primitive. No TX timestamp needed (delay from phase slope, not from t1–t4).

**8. Decisive experiment:** 5 nodes around a room perimeter; place a person at a known `(x, y, z)`; measure the delay matrix with/without the person; reconstruct a 2D horizontal slice; report 3D localisation error vs ground truth. Cheap, uses existing esp-csi + test_harness.py, falsifiable if error > ~2 m.

**9. Ranking:** Novelty high; feasibility high; imaging sharpness moderate (bandwidth-limited); patentability high (white space).

---

## Approach 2 — Reciprocity-Calibrated Interferometric Aperture (RCIA)

**1. Approach name:** Reciprocity-Calibrated Interferometric Aperture (RCIA)

**2. What it images:** Both static structure and people; high-resolution 3D (the sharpest of the incoherent-to-coherent set). Static during the calibration swap required.

**3. Core mechanism / equations:** Escape #2 (OTA reciprocity). For each node pair `(i, j)` measure forward `Ĥ_ij` (i TX, j RX) then reverse `Ĥ_ji` (j TX, i RX) within a channel-coherence interval (static channel). For a reciprocal SISO channel `h_ij = h_ji`, the measured round-trip product

```
Q_ij = Ĥ_ij · Ĥ_ji   →   phase = (θ_i − ψ_j) + (θ_j − ψ_i) − 4π f_k τ_ij
```

exposes the per-node LO combination. Solve the gauge-fixing problem over the connected calibration graph (star is CRB-optimal; O(N) round trips) to estimate per-node TX phase `θ̂_i` and RX phase `ψ̂_j` up to one benign global constant:

```
calibrated phase ϕ_ij = ∠ĥ_ij − (θ̂_i − ψ̂_j)        (now cross-link comparable)
```

With all links sharing one phase reference, form interferometric baselines between links sharing a node and invert via Fourier diffraction tomography for a 3D reflectivity volume. Resolution set by aperture (`λ / (2 · aperture)`) and bandwidth.

**4. Observability analysis:** Requires escape #2 (reciprocity holds during the AB/BA swap — channel static over the ms turnaround) **and** M2 measurable (cross-node phase residual after CFO removal must be stable enough to calibrate). If reciprocity is violated by motion mid-swap, or if M2 fails (residual grows unbounded / random), calibration collapses and RCIA falls back to Approach 1 (MPDT, incoherent). Not usable receive-only; needs `esp_wifi_80211_tx` on both nodes.

**5. Prior-art position:** Reciprocity calibration is established theory (Rogalin/Argos; Zhu star-topology CRLB) and patented (MERL US10547358 family) — moderate FTO. Applying it to ESP32-S3 multi-node 3D aperture synthesis: Consensus found no demonstrated S3 wireless-only reciprocity calibration (M2 unmeasured). **White space:** reciprocity-calibrated ESP32-S3 interferometric 3D imaging.

**6. FTO risk:** Moderate — the reciprocity-calibration patent family (MERL US10547358, expires ~2038) is the closest blocker; design-around by keeping calibration local (star, O(N)) and avoiding broad multi-transceiver occupancy claims. Closure phase remains open (unused here).

**7. ESP32-S3 feasibility:** TX + RX on every node; TDM mandatory; MAC turnaround is milliseconds, so the AB/BA swap must fit inside a phase-coherence interval (tens of ms per note 00/12 if residual CFO ~10–100 Hz). 2.4 GHz. No TX timestamp needed. The go/no-go measurement is M2 (two-S3 residual vs conducted ground truth) — decisive before building RCIA.

**8. Decisive experiment:** 2 nodes; alternate `Ĥ_ij / Ĥ_ji` 1000× with no motion; verify the calibrated relative phase is stable (std < 0.1 rad) over minutes. Then 4 nodes; image a known corner reflector; compare resolution to MPDT. Falsified if residual phase drifts unbounded.

**9. Ranking:** Novelty high; feasibility moderate (gated on M2 + reciprocity); imaging sharpness high; patentability moderate (reciprocity patent exists).

---

## Approach 3 — Near-Field Spherical-Wave Diffraction Tomography (NSDT)

**1. Approach name:** Near-Field Spherical-Wave Diffraction Tomography (NSDT)

**2. What it images:** Static structure primarily (walls, furniture, large objects); people appear as strong scatterers. Both, but best for structure. Works across the whole room including near-field links (< ~1 m at 2.4 GHz, Fraunhofer distance `2D²/λ`).

**3. Core mechanism / equations:** At 2.4 GHz `λ ≈ 12.5 cm`; room-scale links are far-field, but close node–scatterer and node–node links are near-field, so model each TX as a spherical source rather than a plane wave. The scattered field measured at receiver `j` from transmitter `i` via scatterer at `r`:

```
E_scat(k; r_i, r_j) = ∫ σ(r) · G(r_i, r; k) · G(r, r_j; k)  dr
G(a, b; k) = e^{−jk|a−b|} / |a−b|        (spherical-wave Green's function, k = 2πf/c)
```

With multiple transmitter positions `r_i` forming a synthetic aperture, invert the Born approximation by backprojection:

```
σ̂(r) ∝ Σ_{i,j,k} E_scat(k; r_i, r_j) · G*(r_i, r; k) · G*(r, r_j; k)
```

Requires the **complex field** `E_scat` (calibrated phase + amplitude), i.e. it sits on top of RCIA (Approach 2); an amplitude-only fallback is possible but non-convex and low-fidelity.

**4. Observability analysis:** Full complex field needed → requires RCIA calibration (escape #2 + M2). Without calibration only `|E|` and `τ` are available → amplitude-only tomography (much weaker, ill-posed). If M2 fails → amplitude-only NSDT (low fidelity) or fall back to MPDT. Needs static channel over the multi-TX sweep.

**5. Prior-art position:** Radar diffraction tomography is mature; near-field WiFi diffraction tomography is sparse. Consensus found no ESP32-S3 near-field spherical-wave CSI tomography. **White space:** near-field WiFi diffraction tomography with the gauge constraint explicitly handled.

**6. FTO risk:** Low–moderate. Tomographic inversion is general radar prior art; the WiFi-specific application is less patented than occupancy/doppler. Avoid claiming per-zone occupancy (Aerial) — frame as structural imaging.

**7. ESP32-S3 feasibility:** Multi-node complex aperture; needs RCIA first (so M2 gate). 2.4 GHz; near-field model matters for close links. Int8 CSI phase noise limits dynamic range of the inversion. Host-side processing (Approach | in u–w) handles the heavy inversion.

**8. Decisive experiment:** 4 nodes in a tetrahedral arrangement around a volume; calibrate via RCIA; image a metal sphere at a known 3D position; report reconstruction error and resolution vs MPDT.

**9. Ranking:** Novelty high; feasibility moderate–low (depends on RCIA/M2); imaging sharpness high; patentability moderate.

---

## Approach 4 — Passive Ambient Cross-Correlation Tomography (PACCT)

**1. Approach name:** Passive Ambient Cross-Correlation Tomography (PACCT)

**2. What it images:** Static structure (as a stable Green's function) plus people/motion (as time-varying perturbations). Fully passive — images without transmitting anything from Aquila.

**3. Core mechanism / equations:** Receive-only on all 10 nodes; exploit ambient WiFi (neighbour APs, other devices) as incoherent illuminators of opportunity. Record the CSI amplitude envelope `e_j(t) = |ĥ_j(t)|` at every node `j`. Cross-correlate envelopes between node pairs:

```
C_jk(Δt) = ⟨ e_j(t) · e_k(t + Δt) ⟩_t
```

Ambient-noise interferometry: the cross-correlation of a diffuse field between two receivers approximates the Green's function `G_jk` (a virtual source–receiver link). A person moving through the volume perturbs the ambient field, producing correlated envelope changes across many links. Invert the perturbation field by backprojection over all node pairs → 4D (3D + time) image. No emissions from Aquila → privacy-friendly, zero EIRP.

**4. Observability analysis:** Envelope cross-correlation is amplitude-only → fully gauge-invariant. Receive-only → no TX phase at all. **Works even if M2 fails.** Phase cross-correlation would need a gauge fix, but PACCT deliberately uses amplitude envelopes, so it stays on the safe side of the boundary. Static structure = stable `G_jk` (subtracted as baseline); people = correlated time-varying perturbations.

**5. Prior-art position:** Ambient-noise interferometry is mature in seismology; passive WiFi sensing exists (Cognitive/Origin patents — moderate FTO for passive occupancy). Consensus found no receive-only ESP32-S3 ambient-envelope cross-correlation tomography. **White space:** zero-emission ESP32-S3 ambient tomography.

**6. FTO risk:** Moderate — passive occupancy sensing is thickly patented (Aerial US12044789B2, Origin). Receive-only + no dedicated waveform reduces infringement surface; GPLv3 §11 retaliation + defensive publication strategy applies. Frame as structural/ambient, not per-zone occupancy.

**7. ESP32-S3 feasibility:** Ideal — receive-only, no TX, no timestamp sync needed (envelopes are per-node). Depends on ambient traffic density (may be sparse in a quiet home; a single neighbour AP may suffice). 2.4 GHz. Long integration times needed for the cross-correlation to converge.

**8. Decisive experiment:** 4 nodes receive-only in a home; record ambient CSI while a person walks a known route vs an empty room; cross-correlate envelopes; detect and localise the person without transmitting a single Aquila frame.

**9. Ranking:** Novelty moderate; feasibility high (receive-only); imaging sharpness low–moderate (ambient-dependent, slow); patentability low–moderate (passive patents thick).

---

## Approach 5 — Time-Gated Occlusion Tomography (TGOT)

**1. Approach name:** Time-Gated Occlusion Tomography (TGOT)

**2. What it images:** Separates static structure (early paths) from people (late multipath). Images people even when occluded from direct line-of-sight. Both, separable by delay.

**3. Core mechanism / equations:** Use the nanosecond RX timestamp plus the per-link delay `τ_ij` (phase slope) to form a power-delay profile per link:

```
P_ij(τ) = | ℱ⁻¹ { ĥ_ij[k] } |²        (delay profile, τ in seconds)
```

The direct path `τ_direct = |r_i − r_j| / c` arrives earliest → static structure (walls via reflections at characteristic delays). A person's body adds scattered paths at

```
τ_person = (|r_i − r_p| + |r_p − r_j|) / c  >  τ_direct
```

Time-gate the profile:

```
G_static  = [τ_min , τ_direct + Δ]        → static map
G_person  = [τ_direct + Δ , τ_max]        → people / occlusion
```

Late arrivals are triangulated across links by multilateration of `{τ_person,ij}` → 3D position of an occluded person (like a time-of-flight camera, but RF).

**4. Observability analysis:** `τ_ij` from the phase slope is gauge-invariant (absolute group delay). The RX timestamp per node is not even required for `τ` (the slope suffices), though it helps align gates. **Works if M2 fails.** Ambiguity: `τ mod (1/B)` (50 ns / 15 m at 20 MHz) — stitch channels or rely on room < 15 m for single-path; multipath aliases must be resolved by Approach 7.

**5. Prior-art position:** FTM ranging is patented; time-gated multipath imaging for WiFi is sparse. Consensus found no ESP32-S3 CSI delay-gating for occlusion tomography. **White space:** ns-RX-timestamp + CSI delay gating to see through occlusion.

**6. FTO risk:** Low–moderate. Ranging patents exist (FTM) but imaging via time-gating delay profiles is distinct. Avoid FTM-claim overlap; use ESP-NOS CSI phase slope instead.

**7. ESP32-S3 feasibility:** Has the nanosecond RX timestamp (12.5 ns grid) — a real advantage. 20 MHz → 50 ns delay resolution (15 m) raw bin; must interpolate the phase slope for sub-bin precision and stitch channels (Approach 7) to kill ambiguity. Good fit for S3.

**8. Decisive experiment:** 3 nodes; person stands behind a drywall partition (occluded from direct LOS); detect the late-arriving path via time gating; triangulate the 3D position; compare to line-of-sight.

**9. Ranking:** Novelty high; feasibility high; imaging sharpness moderate; patentability moderate–high.

---

## Approach 6 — Doppler Motion Tomography (DMT)

**1. Approach name:** Doppler Motion Tomography (DMT)

**2. What it images:** Motion-only — people, not static structure. Produces a 3D "motion volume" (where motion is occurring), not a structural image.

**3. Core mechanism / equations:** Continuous ping train on each link; per-link phase time series `φ_ij(t)`; remove the static gauge phase → temporal differential `Δφ_ij(t)`. A moving scatterer at `r_p` with velocity `v` on bistatic link `i → j` induces Doppler:

```
f_d,ij = (1/λ) · v · ( û_{i→p} + û_{p→j} )     (bistatic Doppler, λ = c/f_c ≈ 12.5 cm)
Δφ_ij(t) = −2π ∫ f_d,ij(t) dt
```

Static links (no scatterer crossing) show `f_d ≈ 0` and are high-pass filtered out. Links crossing a person show micro-Doppler (gait signature). Invert the Doppler field across all links for a 3D velocity/motion map:

```
motion(r) ∝ Σ_{links through r} | f_d,ij |        (links crossing r light up)
```

**4. Observability analysis:** `Δφ_ij(t)` (time-difference on one link) is gauge-invariant — the static LO phase cancels. Per-link only; **no cross-node coherence needed**. Works if M2 fails. Blind to static scene (standing person invisible) — motion-only by design.

**5. Prior-art position:** WiFi Doppler / micro-Doppler is thickly patented (Origin, Cognitive) — moderate–high FTO. Consensus found no tomographic micro-Doppler (3D motion volume vs point detection) on ESP32-S3. **White space:** 3D motion-volume tomography rather than single-link presence.

**6. FTO risk:** Moderate–high — motion sensing patents are dense. Frame as volumetric motion mapping (not per-person detection/gesture) to narrow the claim scope.

**7. ESP32-S3 feasibility:** Per-link phase time series from int8 CSI; phase noise manageable with averaging. 2.4 GHz `λ = 12.5 cm` → Doppler sensitive to cm/s motion. `v_max = λ/(2T)` ≈ 1.2 m/s at 20 Hz, ~6 m/s at 100 Hz (note 00) — covers walking. No TX needed beyond round-robin pings; receive-only ambient Doppler also possible.

**8. Decisive experiment:** 2-node link; person walks across at known speed/direction; measure micro-Doppler; estimate the crossing point; compare to video/ground truth.

**9. Ranking:** Novelty moderate; feasibility high; imaging sharpness low (motion volume, no structure); patentability moderate (FTO risk tempers it).

---

## Approach 7 — Synthetic-Band Multi-Channel Stitch Tomography (SBMST)

**1. Approach name:** Synthetic-Band Multi-Channel Stitch Tomography (SBMST)

**2. What it images:** Both static structure and people; the bandwidth fix that makes MPDT/TGOT/NSDT sharp enough for 3D.

**3. Core mechanism / equations:** S3 is 2.4 GHz only (channels 1–14, 2412–2484 MHz). Capture CSI on several non-overlapping 20 MHz channels (e.g. 1, 6, 11 → 2412, 2437, 2462 MHz). Per-link phase slope within each channel → delay per channel. Stitch across channels: the synthetic span `B_synth ≈ (2462 − 2412) + 20 ≈ 70 MHz` gives fine range resolution, but the gaps between channels create delay ambiguity. Use a **wide-lane** (the difference between channel centre frequencies, Δf ≈ 25 MHz → wide-lane ambiguity `1/Δf ≈ 40 ns` ≈ 12 m) to disambiguate the fine intra-channel delay (`1/B_ch ≈ 50 ns`), then combine:

```
τ_wl   = τ_ch_a − τ_ch_b        (ambiguous by 1/Δf  → coarse, resolves 12 m)
τ_fine = phase slope within one channel           (precise, 50 ns ambiguous)
τ_abs  = Chinese-remainder-style disambiguation of τ_wl + τ_fine
```

The resulting effective bandwidth sharpens the delay → finer 3D tomography for MPDT/TGOT/NSDT.

**4. Observability analysis:** Per-link phase slope per channel is gauge-invariant, **but stitching across channels needs the relative LO phase between channel hops** — the LO may re-lock with a random phase per boot/channel (ESPARGOS: PLL initial phase re-randomises per channel even with a shared clock). So SBMST needs either (a) a channel-hop phase calibration on a reference link, or (b) to treat channels independently (no stitch → coarse). If M2 fails / LO hop phase is random → per-channel tomography only (7.5 m bin), or wide-lane only (12 m ambiguous). Partially gated on M2 / channel-hop phase stability.

**5. Prior-art position:** 802.11bf multi-band fusion (MERL US11902811) and channel-bonding CSI exist — moderate FTO. Consensus found no narrowband-only ESP32 wide-lane stitch tomography. **White space:** 2.4-GHz-only synthetic-band stitching with wide-lane disambiguation.

**6. FTO risk:** Moderate (MERL multi-band patent). Narrowband channel-stitching as a delay disambiguation technique is less clearly claimed.

**7. ESP32-S3 feasibility:** Channel hopping is slow (ms) and CSI must be captured per channel; 2.4 GHz only; 20/40 MHz. 40 MHz channels reduce the number of stitchable channels. ESP-NOS HT40 MCS0 ping per channel. No TX timestamp needed.

**8. Decisive experiment:** 2 nodes; place a target at a known range 3–10 m; estimate range via single channel (20 MHz, ambiguous) vs stitched wide-lane; report error vs ground truth.

**9. Ranking:** Novelty moderate; feasibility moderate; imaging sharpness moderate–high (enables the others); patentability moderate.

---

## Approach 8 — Vertical Stairwell Mode Tomography (VSMT)

**1. Approach name:** Vertical Stairwell Mode Tomography (VSMT)

**2. What it images:** Cross-floor / vertical (`z`) structure — people and static structure in and around a stairwell across multiple floors. Complements horizontal perimeter imaging.

**3. Core mechanism / equations:** Place nodes on multiple floors and in/adjacent to the stairwell. A stairwell is a vertical shaft → strong vertical multipath with guided/resonant paths. Measure per-link delay `τ_ij` and amplitude `ρ_ij` between floor nodes. A person in the stairwell perturbs the stairwell-guided paths, shifting delay/amplitude in a way localised to a vertical band. The vertical aperture (nodes at different `z`) resolves elevation:

```
Δτ_ij(z) ≈ perturbation of stairwell-guided path on link (i,j) when person at height z
occupancy(z) ∝ Σ_{links crossing stairwell at z} | Δτ_ij | + | Δρ_ij |
```

The stairwell also acts as a partial common vertical reference path (escape #1 flavour) linking floors, though cross-floor phase coherence still needs an escape; VSMT deliberately uses delay/amplitude (gauge-invariant) so it does not depend on phase coherence.

**4. Observability analysis:** Delay + amplitude per link are gauge-invariant. **Works if M2 fails.** Cross-floor phase coherence is NOT required (we use delay/amplitude, not complex cross-floor phase). Floor attenuation at 2.4 GHz is the real limiter — the stairwell's open vertical path mitigates it.

**5. Prior-art position:** Invention Z (cross-floor coherent aperture via stairwell reference) is related but different: Z uses the stairwell as a *coherence reference*; VSMT uses it as a *resonant/scatterer vertical aperture*. Consensus found no stairwell-mode tomography. **White space:** vertical stairwell resonator tomography for cross-floor 3D imaging.

**6. FTO risk:** Low–moderate. Cross-floor sensing is less densely patented than single-floor occupancy. Not a closure-phase claim.

**7. ESP32-S3 feasibility:** Multi-floor deployment; 2.4 GHz penetrates floors poorly — stairwell helps. Receive-only or round-robin. No TX timestamp needed. Good fit if a multi-floor test home is available.

**8. Decisive experiment:** 3 floors; nodes in the stairwell plus each floor; person moves in the stairwell on one floor; detect vertical position `z` via delay/amplitude perturbation on stairwell links; compare to known floor.

**9. Ranking:** Novelty moderate; feasibility moderate (depends on multi-floor access + floor attenuation); imaging sharpness low–moderate (vertical only); patentability moderate.

---

## Approach 9 — Coherent-Change-Detection Volume Tomography (CCDVT)

**1. Approach name:** Coherent-Change-Detection Volume Tomography (CCDVT)

**2. What it images:** People / occupancy volume; static structure is subtracted out. Motion/people only (like DMT but amplitude/delay based, works for slow or stationary people).

**3. Core mechanism / equations:** Acquire a baseline empty-room CSI per link: amplitude `ρ⁰_ij` and delay `τ⁰_ij`. During operation measure `ρ_ij`, `τ_ij`. A person occupies voxel `v` iff the links passing through `v` show correlated coherent change:

```
Δρ_ij = ρ_ij − ρ⁰_ij       Δτ_ij = τ_ij − τ⁰_ij
```

Score each voxel by the links whose straight (or Eikonal) path crosses it:

```
occupancy(v) = σ( Σ_{(i,j): link(i,j) crosses v} [ α · Δρ_ij / ρ⁰_ij + β · Δτ_ij / τ⁰_ij ] )
```

`σ` = sigmoid. Static structure = baseline (subtracted). A stationary person still produces a coherent change across the links crossing their body → occupancy volume, not just motion. 3D because only links passing through a voxel are perturbed.

**4. Observability analysis:** `Δρ`, `Δτ` per link are gauge-invariant (baseline removes the static `θ_i − ψ_j`). No cross-node coherence. **Works if M2 fails.** Static scene invisible by design (subtracted); slow or stationary people visible (unlike DMT).

**5. Prior-art position:** Link-based occupancy detection is heavily patented (Aerial, Origin) — moderate–high FTO. Consensus found no volumetric occupancy tomography from coherent per-link perturbation on ESP32-S3. **White space:** 3D occupancy volume from delay+amplitude link perturbation (not binary per-zone occupancy).

**6. FTO risk:** Moderate–high (occupancy patents thick). Frame as structural-change tomography, not per-person count/identity, to narrow scope.

**7. ESP32-S3 feasibility:** Simple — per-link amplitude + delay only; software builds on MPDT infrastructure. 2.4 GHz; receive or round-robin. Best first software layer after MPDT.

**8. Decisive experiment:** 6 nodes around a room perimeter; person placed at a known voxel; compute the occupancy volume; report voxel localisation error vs ground truth, with and without the person holding still.

**9. Ranking:** Novelty moderate; feasibility high; imaging sharpness low–moderate (occupancy, not high-resolution image); patentability moderate (FTO risk).

---

## Ranked Summary Table

| Rank | Approach | Images | Gauge-safe? | Needs M2 / escape | Sharpness | Feasibility (S3) | FTO risk | Patentability |
|:--:|---|---|---|---|---|---|---|---|
| 1 | MPDT | structure + people | Yes | None (M2-irrelevant) | Moderate | High | Low–mod | High |
| 2 | TGOT | structure vs people (occlusion) | Yes | None | Moderate | High | Low–mod | Mod–high |
| 3 | PACCT | structure + people (passive) | Yes (amp only) | None (receive-only) | Low–mod | High | Mod | Low–mod |
| 4 | RCIA | structure + people (high-res) | No | Escape #2 + M2 | High | Mod | Mod | Moderate |
| 5 | DMT | motion volume | Yes | None | Low | High | Mod–high | Moderate |
| 6 | NSDT | structure + scatterers | No | Escape #2 + M2 | High | Mod–low | Low–mod | Moderate |
| 7 | SBMST | structure + people | Partial* | Channel-hop phase | Mod–high | Moderate | Moderate | Moderate |
| 8 | CCDVT | occupancy volume | Yes | None | Low–mod | High | Mod–high | Moderate |
| 9 | VSMT | cross-floor vertical | Yes | None | Low–mod | Moderate | Low–mod | Moderate |

\* SBMST is gauge-safe per channel but stitching across channels needs LO hop-phase stability (not fully M2, but a related coherence question).

### How to read the ranking
- **Gauge-safe** = works within the `U(1)` boundary, no added reference, robust if M2 fails. These (MPDT, TGOT, PACCT, DMT, CCDVT, VSMT) are the safe first builds.
- **Needs M2/escape** = RCIA, NSDT (reciprocity + M2); SBMST (channel-hop phase). These are the coherent high-sharpness tier and are gated on the M2 measurement.

## Recommended 10-Node Hardware Campaign Sequencing

**Build 1 — MPDT (the core).** Round-robin 5–10 nodes around a floor perimeter; measure the delay + amplitude matrix; reconstruct 2D/3D slices of people and static structure. Purpose: prove 3D imaging is possible on ESP32-S3 *without* any cross-node phase coherence, so the campaign does not depend on the unmeasured M2. Uses existing esp-csi + test_harness.py. Build first because it de-risks everything else.

**Build 2 — TGOT (occlusion).** Add the nanosecond RX timestamp and per-link delay gating; demonstrate detection of a person behind a drywall partition (late arrivals triangulated). Purpose: validate the S3 RX timestamp in anger and show people are visible even when occluded — the strongest differentiator vs camera-free rivals.

**Build 3 — PACCT (passive / privacy).** Run receive-only with ambient WiFi; cross-correlate envelopes; detect a person walking without transmitting any Aquila frame. Purpose: establish the zero-emission, privacy-first operating mode (aligns with invention U) and confirm receive-only feasibility before any TX is enabled.

**Gate — measure M2.** Two S3s, alternate `Ĥ_ij / Ĥ_ji`, compare calibrated relative phase vs a conducted ground truth. **Falsified if residual grows unbounded or is random.** This single measurement decides whether the campaign may proceed to coherent modes.

**Build 4 — RCIA → NSDT (coherent high-resolution), only if M2 passes.** Reciprocity-calibrate a star (O(N) round trips), form the interferometric aperture, and run near-field diffraction tomography for sub-metre 3D. Purpose: the sharpest imaging; conditional on M2.

**Build 5 — DMT + CCDVT (motion/occupancy layers).** Software layers on top of MPDT infrastructure: Doppler motion volume (DMT) and coherent-change occupancy volume (CCDVT). Purpose: motion detection and stationary-person occupancy, complementing structural imaging.

**Build 6 — SBMST (bandwidth) and VSMT (cross-floor).** Channel stitching + wide-lane to sharpen MPDT/TGOT/NSDT range resolution; then a multi-floor stairwell deployment for vertical imaging. Purpose: extend resolution and reach cross-floor (Track 3 geometry), last because they need either M2 (SBMST stitch) or multi-floor access (VSMT).

**Ordering rationale:** Everything before the M2 gate is gauge-safe and works if M2 fails. RCIA/NSDT/SBMST-stitch are the coherent tier and are deliberately last, so a negative M2 result still leaves a complete, useful imaging system (MPDT + TGOT + PACCT + DMT + CCDVT). This matches the v2 destination: learned route, geometry demoted to Track 3, placement structured (≥3 nodes/floor perimeter), camera-free runtime, per-home adaptation via git PRs.

---

### Relation to existing inventions
These approaches sit alongside, not in place of, inventions a–t and u–w:
- MPDT/TGOT/CCDVT can consume the **closure-phase ledger** (inventions a–t) as an optional coherence layer when available, but do not require it.
- PACCT operationalises invention U's receive-only, gauge-free principle at the imaging level.
- TGOT consumes the nanosecond RX timestamp (note 00 §8) that superseded the v1 "no PHY timestamp" stance.
- RCIA/NSDT are the coherent destination that invention W's commissioning and invention {/'s edge/host partition would support once M2 is resolved.
- None of them require 802.11bf procedures or cross-node absolute phase — keeping the FTO profile low and the implementation on legacy ESP32-S3 CSI capture.
