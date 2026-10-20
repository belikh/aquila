# 13 — Static-multipath and dominant-path self-referencing

**Question.** Can a static multipath component or dominant LoS path act as a
self-reference to stabilise CSI over time, and how is the static part
separated? Required coverage: (a) static/dynamic decomposition; (b) dominant-
path/LoS detection; (c) temporal self-referencing (background subtraction,
PCA); (d) stability limits.

Australian English. Evidence tags: **ESTABLISHED / HYPOTHESIS / SPECULATION**.
Hardware honesty kept explicit.

---

## Findings

### (a) Static/dynamic decomposition

**ESTABLISHED.** The standard model writes the frequency-domain CSI of frame
`p` on subcarrier `k` as an additive superposition

```
h_{p,k} = b_k + d_{p,k} + n_{p,k}
```

where `b_k` is the **static** component (the direct/LoS path plus reflections
off walls, floor and furniture that do not change over the observation window)
and `d_{p,k}` is the **dynamic** component carrying the motion to be sensed.
This is the textbook decomposition across the Wi-Fi sensing literature and is
formalised with a static-power fraction `γ = Σ|b_k|²/K` in the Samsung
preprocessing paper (Ratnam et al., arXiv:2307.12126). FarSense (Zeng et al.,
IMWUT 2019) uses the same grouping into "static path + dynamic path". MORIC
(arXiv:2506.12997) refines it by transforming CSI into the **delay profile**
space and splitting it into per-delay components, each with its own Doppler;
it explicitly notes the dominant LoS component `|β₀|` *overshadows* weaker
motion paths, which is precisely why per-path separation is needed.

The important nuance for Aquila: the static component is *not* a single path.
It is an aggregate of the LoS path and every fixed scatterer, and it is only
"static" relative to the sensing timescale — any drift of the environment or
the nodes moves power between `b_k` and `d_{p,k}`.

### (b) Dominant-path / LoS detection

**ESTABLISHED.** In the delay domain the LoS path is the **earliest-arriving**
and (in a clear indoor link) the **strongest** component. Two peer-reviewed
systems use exactly this to build a self-reference:

- **LoSRef** (arXiv:2602.05344, IEEE 802.11ax, COTS Intel AX210, 160 MHz)
  identifies the dominant CIR peak, *assumes* it is the LoS path, and aligns
  its delay to the known Tx–Rx distance `d_ref = d_Tx–Rx`. This delay
  calibration plus a phase rotation `e^{−jθ*}` ("phase alignment") puts every
  multipath component on a common delay/phase axis relative to the LoS path.
  It needs only a **single-antenna, single-port receiver** — unlike wired-
  reference or reference-antenna designs — and it recovers target dynamics
  "up to 20 dB weaker than dominant static multipath components".
- **JUMP** (Pegoraro et al., arXiv:2304.07766, IEEE TWC 2024; 60 GHz
  IEEE 802.11ay) removes the carrier frequency offset (CFO) by tracking the
  phase of a **dynamically-selected static reference path**. Its selection
  procedure: track reflectors over `K` frames, keep those whose location
  variance is below a threshold `σ²_thr` (it uses 0.005 m² over K=100), and
  pick the **strongest** stable one. The reference may be the LoS path *or* a
  fixed reflection (e.g. a metal locker), so JUMP tolerates intermittent LoS
  blockage. It reports that using the LoS path gives the best CFO estimate
  because it is stronger than any first-order reflection.

The underlying physics that makes this work is stated identically in both:
**CFO is common to all multipath components of a packet**, so a
static path's phase measures the CFO and nothing else, and can be divided out
of the sensing paths. This is the same gauge argument Aquila uses elsewhere
(the common factor cancels), applied *inside one link and one packet*.

**ESTABLISHED (alternative reference).** The two-antenna **CSI ratio**
(FarSense, Zeng et al., IMWUT 2019, DOI 10.1145/3351279) cancels the
time-varying random phase offset because both antennas share one RF
oscillator: `H_r = H₁/H₂` removes `e^{−jθ_offset}` and leaves a stable phase.
The general **CSI-ratio model** is surveyed in Wu, Zeng, Zhang & Zhang, CCF
Trans. Pervasive Comp. Interact. 4(1):88–102, 2022,
DOI 10.1007/s42486-021-00077-z. This is a *hardware* reference (shared LO
across antennas) rather than a *path* reference, and it is complementary to
LoSRef: ratio removes the LO term, LoSRef removes the residual CFO/delay
ambiguity of a single-port receiver.

### (c) Temporal self-referencing: background subtraction and PCA

**ESTABLISHED.** Static removal by temporal differencing and by
Principal Component Analysis / Robust PCA (low-rank background + sparse
foreground) is the long-standing method in both video and RF sensing
(ICA/PCA background subtraction: PubMed 19095527; RPCA survey work by
Bouwmans et al.). Wi-Track removes wall and static-object reflections by
subtracting consecutive frames.

**ESTABLISHED (caveat).** LoSRef explicitly reports that **PCA on the CFR
magnitude is unreliable** as a self-reference: "PC1 frequently reflects
dominant static or hardware-related variations rather than motion-induced
dynamics", so PC1 is not guaranteed to be the motion component. This is the
decisive argument for *path-domain* referencing (LoSRef/JUMP) over blind PCA:
PCA discovers the subspace, it does not *label* which component is static, and
the strongest principal component is often the static/hardware term you want to
reject.

### (d) Stability limits

**ESTABLISHED (limits, from the same papers).**

- LoSRef *assumes* the LoS component "remains dominant and well resolvable";
  scenarios where it does not (target extremely close to the Tx–Rx pair) are
  declared out of scope. A blocked/diffuse LoS breaks the delay reference.
- JUMP shows the reference can be a static reflection when LoS is occluded,
  but the fallback requires a *strong* stable scatterer; "with few static
  reflections" the CFO cannot be removed.
- JUMP's key limitation, and the most important for Aquila: **the TX and RX
  must be static.** If the RX moves, the Doppler along the Tx–Rx path differs
  from the Doppler along the target–RX path (different angles `η` and `ξ`), so
  the reference phase no longer contains only CFO and cannot be divided out.
- AsyMov (arXiv:2412.10387) attacks this moving-node case directly, estimating
  and compensating per-path phase offsets for moving Tx and Rx; it confirms
  that path-per-path Doppler differs and must be modelled.
- All of the above hold only over the coherence window of the static channel.
  Environment drift (doors, wind, foliage — e.g. arXiv:2606.07347 orchard
  measurements) changes `b_k` on seconds-to-minutes timescales.

---

## ESP32-S3 feasibility

**S3 phase is the bottleneck, not the algorithm.** On the ESP32-S3, esp-csi
delivers quantised 802.11n CSI over 20/40 MHz (≈64 subcarriers); the raw CSI
phase is corrupted by CFO and sampling-frequency offset (SFO), the same error
terms LoSRef/JUMP exist to remove. Espressif's own reference solution does
**not** claim OTA phase coherence: its motion-detection reference design
carries a **common-crystal (co-crystal) link** — a clock buffer between an
ESP32-C3 and ESP32-S3 — explicitly "to eliminate the relative frequency offset
caused by the clock asynchrony between the two chips" so that "phase
information in the CSI data" becomes usable (ESP-Techpedia, ESP-CSI Solution).
That is a *hardware* reference, not the OTA self-reference this prompt asks
about. Community ESP32-S3 builds likewise discard phase and fight channel
drift (ruvnet/RuView discussion #1416). The one recent ESP32 phase paper
(arXiv:2507.12854, static person ID) preprocesses phase (removing SFO/CFO
terms) but does not use a LoS-reference and does not claim cross-frame phase
coherence.

Bandwidth is a second, quantifiable gap. LoSRef demonstrates at **160 MHz**
(802.11ax) → 6.3 ns delay resolution; the paper itself notes that 20/40 MHz
802.11n gives "a less distinct delay-domain structure" and a looser Cramér–Rao
bound on delay. ESP32-S3 CSI is 20/40 MHz, so the LoS peak is harder to
resolve from close-in multipath. 2.4 GHz (λ ≈ 12.5 cm) also gives less phase
accumulation per metre than the 5/6 GHz bands used by FarSense-class work.

**Verdict:** OTA dominant-path self-referencing is *plausible* but
**not demonstrated on ESP32-S3**, and the S3's native phase incoherence plus
narrow bandwidth are exactly the conditions LoSRef/JUMP were built to survive
— so this is a high-value, testable gap rather than an established capability.

**TX needed?** Not *additional* TX. A source is assumed anyway: either an
ambient AP (passive receive) or one Aquila node transmitting compliant frames
(via `esp_wifi_80211_tx`). The LoS path of that existing link *is* the
reference. The **ping-and-unique-reply** round trip is naturally compatible:
each reply's LoS component can serve as the per-direction delay/phase
reference, and its unique identifier attributes the round trip. But the
reference is not free — it requires physically stationary nodes with a clear
(or strong-static-reflector) path, and a known short baseline for absolute
delay. Cross-*node* coherent phase is still not obtained this way: LoSRef
gives phase *relative to each link's own LoS path*, which is a per-link gauge,
not a network-wide one.

---

## Evidence strength

- **Dominant/static-path self-referencing on COTS Wi-Fi: 8/10.** Two
  independent peer-reviewed systems (LoSRef, IEEE 802.11ax; JUMP, IEEE
  802.11ay/TWC), with explicit residual-error analysis, quantify it. CSI-ratio
  (FarSense) independently confirms the shared-oscillator cancellation.
- **Same capability on ESP32-S3: 2/10.** No published demonstration; native
  phase requires a conducted co-crystal reference in Espressif's own design.

Overall for the *question as posed for Aquila*: **6/10** — the mechanism is
real and well-characterised on other COTS hardware, but the ESP32-S3
instantiation is unproven and bandwidth-limited, and it does not by itself
cross the independent-LO unobservability boundary for cross-node phase.

---

## Primary sources

1. **LoSRef** — "Wi-Fi Radar via Over-the-Air Referencing: Bridging Wi-Fi
   Sensing and Bistatic Radar", arXiv:2602.05344 (v3).
   https://arxiv.org/abs/2602.05344 — OTA LoS-path delay calibration + phase
   alignment, COTS Intel AX210, 160 MHz 802.11ax.
2. **JUMP** — Pegoraro et al., "JUMP: Joint Communication and Sensing With
   Unsynchronized Transceivers Made Practical", IEEE TWC 2024,
   arXiv:2304.07766. https://arxiv.org/abs/2304.07766 — CFO removal via phase
   of a dynamically-selected static reference path; 60 GHz 802.11ay.
3. **FarSense** — Zeng, Wu, Xiong, Yi, Gao, Zhang, IMWUT 3(3), 2019,
   DOI 10.1145/3351279. https://arxiv.org/abs/1907.03994 — two-antenna CSI
   ratio cancels the random phase offset.
4. **CSI-ratio model survey** — Wu, Zeng, Zhang, Zhang, CCF Trans. Pervasive
   Comp. Interact. 4(1):88–102, 2022, DOI 10.1007/s42486-021-00077-z.
5. **Optimal preprocessing of WiFi CSI** — Ratnam et al., arXiv:2307.12126 —
   static `b_k` / dynamic `d_{p,k}` model, strongly-LoS and strongly-static
   estimators, γ. https://arxiv.org/abs/2307.12126
6. **MORIC** — "CSI Delay-Doppler Decomposition for Robust Wi-Fi-based HAR",
   arXiv:2506.12997. https://arxiv.org/abs/2506.12997
7. **Widar2.0** — Qian et al., MobiSys 2018, DOI 10.1145/3210240.3210314 —
   dynamic path-length-change-rate / Doppler extraction from CSI.
8. **AsyMov** — "Integrated Sensing and Communications with moving nodes",
   arXiv:2412.10387. https://arxiv.org/abs/2412.10387 — LoS-path reference
   under moving Tx/Rx.
9. **π-Splicer** — Zhu et al., IEEE TMC 17(9):2155–2165, 2018 — accurate CSI
   phase from commodity Wi-Fi (phase sanitisation).
10. **Exposing the CSI** — arXiv:2302.00992 — 802.11ax CSI sensing limits;
    MIMO ≥ bandwidth for sensing.
11. **Espressif ESP-CSI Solution** (ESP-Techpedia) —
    https://docs.espressif.com/projects/esp-techpedia/en/latest/esp-friends/solution-introduction/esp-csi/esp-csi-solution.html
    — co-crystal clock-buffer link required for usable phase.
12. **ESP32 static person ID** — arXiv:2507.12854 — ESP32 TX/RX phase
    preprocessing, SFO/CFO terms.

---

## Residual unknowns

- No measurement of **residual phase error after LoSRef on a 20/40 MHz,
  2.4 GHz ESP32-S3 link**, versus the S3's idle phase noise. This is the
  single number that decides feasibility.
- Whether the S3's esp-csi **quantisation and per-frame gain/AGC** allow a
  temporally stable CIR peak at all, or whether AGC steps masquerade as
  "static" variation.
- Whether a **strong static reflector** (metal object) is reliably available in
  a typical Aquila room to serve as the fallback reference.
- **Cross-node**: whether two receivers' LoS phases to one common transmitter
  can be combined into a network-wide delay/phase gauge (bridges prompt 11);
  unaddressed by any located source for ESP32-class hardware.
- JUMP's static-Tx/Rx assumption versus a **drifted or deliberately moving**
  polyhedron node.

---

## (A) Chaining

- **TX capability (3) enables two-way LoS self-referencing.** A stationary
  Aquila node transmitting compliant frames creates the LoS path that each
  receiver uses as its delay/phase reference. Without a transmitted reference
  frame there is no deterministic LoS component to lock to (you fall back to
  ambient traffic, which may not carry the structure you need).
- **LoS self-referencing enables phase-coherent CSI (partial).** It converts
  the S3's incoherent raw phase into phase *relative to a stable path*,
  enabling delay–Doppler, micro-Doppler and slow-motion extraction that raw
  phase cannot support.
- **It does not replace OTA reciprocity (prompt 10) or a common-transmitter
  reference (prompt 11) for cross-node phase.** LoSRef is a per-link gauge.
  The highest-leverage enabler remains the *common transmitter*: it turns each
  node's LoS path into a reference to the *same* physical clock, which is what
  would give cross-node coherence. LoSRef then rides on top of it.
- **Blocked without:** a stationary link and either a resolvable LoS path or a
  strong static reflector. Blocked entirely if nodes move (JUMP limitation),
  unless the AsyMov-style per-path compensation is added.

## (B) Existing models and data

- **LoSRef, JUMP, FarSense, CSI-ratio model** — model-based, no training, open
  in the literature; directly usable as the algorithm. Input format: complex
  CFR/CIR per packet (esp-csi provides complex CSI, so format-compatible in
  principle).
- **Widar2.0, MORIC** — dynamic-path/Doppler feature extractors; model-based,
  applicable after LoS-referencing.
- **π-Splicer** — phase sanitisation preprocessing, directly relevant to S3.
- **LICENCE/HW caveat:** these are papers (methods), not licensed code; no
  public ESP32 implementation of LoSRef/JUMP exists. Existing ESP32 CSI
  datasets (e.g. the ESP-CSI-tool RF HAR dataset, Data in Brief,
  S2352340924010631) contain amplitude/CSI but do **not** record
  phase-coherent LoS-referenced data, so they cannot validate this method.

## (C) Own-data / new technology

**Could a purpose-built dataset unlock something not demonstrated?** Yes — a
**phase-coherent ESP32-S3 LoS-reference dataset** would be a first. Minimal
experiment:

- Two stationary ESP32-S3 nodes at a known short baseline (e.g. 2 m, clear
  LoS), 20 MHz and 40 MHz, one TX sending frames at a fixed rate; record
  complex CSI plus host reference timestamps. Introduce a single moving
  scatterer (a person) on a repeatable path and, in a control run, no target.
- **Measure:** residual frame-to-frame phase error of the CIR after
  LoSRef-style alignment (delay + phase), as a function of SNR and time;
  compare against (i) raw phase and (ii) a co-crystal (conducted shared-clock)
  ground-truth built per Espressif's reference design. Report the residual in
  degrees and the equivalent delay error versus the 6.3 ns / 50 ns resolution
  bound.
- **Prove:** that on the S3 the dominant-path reference yields a *stable*
  relative-phase CIR over ≥ minutes with residual phase below the motion
  signal of interest.
- **Falsify:** if post-reference residual phase error is statistically
  indistinguishable from the S3's idle phase noise, or if AGC/quantisation
  events dominate the "static" component, the method fails on this hardware —
  and the co-crystal hardware reference becomes the only ESP32 route to
  coherent phase.

That single experiment resolves the decisive residual unknown and gives Aquila
a defensible answer on whether it must fall back to a conducted/shared-clock
reference for any phase-coherent sensing on ESP32-S3.
