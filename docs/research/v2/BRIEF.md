# Aquila v2 — 20-agent deep-research brief

This file is read by 20 parallel research agents. Each agent executes **one**
numbered prompt, writes a note to `docs/research/v2/<NN>-<slug>.md`, and returns
a bounded summary. Australian English.

## Shared context (every agent)

You are one of 20 parallel deep-research agents for **Aquila**
(github.com/belikh/aquila), a WiFi CSI sensing project. Reference hardware:
**ESP32-S3**. Use **primary sources only**; separate **ESTABLISHED / HYPOTHESIS /
SPECULATION**; flag anything you cannot verify.

**Research tools.** Use the Parallel plugin tools: `parallel-search` (objective +
2–3 focused `search_queries`) to find sources, then `parallel-fetch` (urls +
objective) to extract from the best ones. Iterate — layer searches, follow
citations to primaries (papers, datasheets, standards, upstream code). Prefer
primaries over blog posts. If those tools are unavailable, use your own web tools.

**Scope — three radio capabilities:**
1. `esp-csi` CSI mode (per-802.11-frame channel state information; whole ESP32 family).
2. Experimental raw-I/Q receive mode (ESPARGOS / esp-sdr): receive-only, gapped, uncalibrated.
3. Transmission of compliant 802.11 frames (`esp_wifi_80211_tx`) by one or many
   nodes, sequentially or concurrently; clusters/groups of senders and receivers.
   Include volumetric/spherical geometries — e.g. a **soldered-PCB polyhedron**
   (icosahedron = 20 triangular faces = 20 nodes) as a compact 3D aperture.

**Strategic framing — address all three explicitly in your note:**
- **(A) Chaining.** Which capabilities enable which others. State dependencies
  ("X enables Y"; "Y blocked without Z"). Identify the highest-leverage enabling
  capability and any capability that unlocks a further one.
- **(B) Existing models/data.** Name published ML models, public datasets, and
  open-source implementations that already address this (with sources), and
  whether Aquila can use them (licence, hardware, input format).
- **(C) Own-data / new technology.** Could a purpose-built Aquila dataset or
  experiment unlock something not currently demonstrated? Give the minimal
  experiment/dataset that would prove it and what would falsify it.

**Also consider** the round-trip scheme: one node pings; every other node replies
with a **unique** packet; the round trips are timed. What can this measure
(offset, range, Doppler, identity, channel), and where does it break?

**Hardware honesty.** State whether a finding is demonstrated on **ESP32-S3 /
ESP32 family** specifically, on **other COTS Wi-Fi**, or only on **SDR-grade**
hardware — and quantify the gap.

**Assume (do not re-litigate; you may challenge with sources):** independent-LO,
receive-only, no-common-reference cross-node phase is unobservable; the three
published escapes are a common-transmitter reference, OTA reciprocity
(`Ĥ_AB·Ĥ_BA⁻¹` cancels the channel), and a shared/conducted reference; none of the
wireless-only escapes is yet proven on ESP32-S3; non-coherent temporal CSI ratios
give Doppler (moving only, blind to static); FTM (802.11mc) is supported on S3 but
coarse; the MAC exposes a microsecond RX timestamp.

**Output.**
1. Write your full note to `/home/matt/projects/aquila-v2-spec/docs/research/v2/<NN>-<slug>.md`
   (create parent dirs). Structure: Question; Findings by
   ESTABLISHED/HYPOTHESIS/SPECULATION; ESP32-S3 feasibility; TX needed?; evidence
   strength (1–10) with justification; primary sources with DOIs/URLs; residual
   unknowns; and the Chaining / Existing-models-and-data / Own-data sections.
2. Return, as your **only** final message, a **≤300-word** summary: exact file
   path, 3–5 key findings, ESP32-S3 feasibility, evidence strength, and the single
   most decisive next experiment. Do **not** paste the full note.

---

## The 20 prompts

### 01 — Single-node passive CSI sensing
`01-single-node-passive-csi`
Q: What can one ESP32-S3 receiver infer from 802.11 CSI alone about motion,
presence and amplitude/Doppler, and what bounds detection? Must contain:
(a) amplitude/phase features used; (b) temporal CSI ratios and their blindness to
static scatter; (c) single-link detection range/rate limits; (d) sensitivity to
AGC, per-frame rate, null-subcarriers. Leads: esp-csi (Espressif), Wi-Fi sensing
surveys, PhaseBeat, TensorBeat. Overlap: not 02, 04, 06.

### 02 — Multi-node passive localisation and fingerprinting
`02-multinode-passive-localisation`
Q: With several passive receivers, how do CSI/RSSI fingerprints localise
uncooperative people, and what accuracy do device-free methods achieve? Must
contain: (a) radio tomographic imaging (RTI) formulation; (b) fingerprinting vs
model-based; (c) node count/placement effects; (d) NLoS and multi-person
ambiguity; (e) calibration burden. Leads: Wilson & Patwari (RTI), E-eyes, CARML.
Overlap: not 01, 03, 15, 16.

### 03 — Channel charting from unlabelled CSI
`03-channel-charting`
Q: Can channel charting recover relative geometry from unlabelled CSI, and what
does it need that Aquila lacks? Must contain: (a) pipeline (dim-reduction +
neighbour embedding); (b) assumptions (anchors, temporal continuity); (c) metric
vs geographic distortion; (d) labels needed for absolute alignment. Leads: Studer
et al. "Channel Charting"; ESPARGOS channel-charting repos. Overlap: not 02, 16.

### 04 — Through-wall/NLoS CSI and material inference
`04-through-wall-materials`
Q: How do walls and materials attenuate and shape CSI, and what can be inferred
about NLoS geometry and material type? Must contain: (a) attenuation vs frequency
for concrete/plaster/glass/water; (b) through-wall range/resolution limits;
(c) direct vs multipath separation; (d) specular vs diffuse modelling. Leads:
WiVi (Adib & Katabi), RF-Capture, ITU-R P.2040. Overlap: not 01, 13, 14.

### 05 — Raw I/Q fundamentals (esp-sdr / ESPARGOS)
`05-raw-iq-fundamentals`
Q: What does raw baseband I/Q observe that CSI does not, and what do experimental
ESP32-class I/Q receivers actually deliver? Must contain: (a) I/Q vs CSI
information content; (b) ESPARGOS/esp-sdr architecture, bandwidth, sample rate,
duty/gaps; (c) calibration status; (d) demonstrated capabilities and limits.
Leads: ESPARGOS papers (arXiv 2502.09405, 2408.16377), esp-sdr, GNU Radio.
Overlap: not 06, 01.

### 06 — I/Q passive/bistatic radar and pulse-Doppler
`06-iq-passive-radar`
Q: Can gapped, uncalibrated ESP32 I/Q support passive/bistatic radar and
pulse-Doppler detection, and what are the range/velocity limits? Must contain:
(a) pulse-Doppler processing and resolution; (b) gaps/duty effect on unambiguous
range/velocity; (c) passive (illuminator-of-opportunity) vs bistatic geometry;
(d) required integration gain. Leads: Skolnik; Willis (Bistatic Radar); Wi-Fi/OFDM
passive radar. Overlap: not 05, 08.

### 07 — Legal envelope and `esp_wifi_80211_tx` primitives
`07-tx-legal-primitives`
Q: What 802.11 frames can an S3 lawfully transmit under ACMA LIPD / FCC Part 15,
and what primitives does `esp_wifi_80211_tx` expose? Must contain: (a) the API —
frame types, ACK/block-ACK, injection, rate/bandwidth knobs; (b) ACMA LIPD class
licence EIRP/duty at 2.4 GHz; (c) FCC 15.247 dwell/PSD; (d) disallowed frames.
Leads: ESP-IDF `esp_wifi_80211_tx` docs; ACMA LIPD Class Licence 2015; 47 CFR
15.247; ETSI EN 300 328. Overlap: not 08, 09.

### 08 — Monostatic/bistatic/multistatic radar geometries
`08-radar-geometries`
Q: With compliant 802.11 TX, which mono-, bi- and multistatic geometries are
achievable — including **volumetric arrays (soldered-PCB polyhedra, geodesic/
spherical arrays)** — and how do synchronisation and self-interference constrain
them? Must contain: (a) geometry taxonomy and coverage; (b) required sync per
mode; (c) monostatic self-interference; (d) waveform/duty design; (e) cluster
spectral sharing; (f) spherical/volumetric vs planar array benefits for 3D DoA.
Leads: Skolnik; Chernyak; geodesic/spherical array literature; ESPARGOS. Overlap:
not 06, 07, 09.

### 09 — MIMO multiplexing, TDM necessity, full-duplex
`09-mimo-tdm-fullduplex`
Q: Across sender/receiver clusters, where is TDM mandatory vs FDM/CDM viable, and
is single-channel full-duplex feasible on ESP32? Must contain: (a) TDM/FDM/CDM
trade-offs; (b) half-duplex self-interference and leakage limits; (c) why TDM is
required under a shared LO; (d) simultaneous-transmission legality/co-channel
interference; (e) whether a co-located **polyhedron** array with a shared clock
removes the TDM requirement for AoA. Leads: Argos (Rice, MobiCom 2012);
full-duplex Wi-Fi / RF self-interference-cancellation surveys. Overlap: not 08, 12.

### 10 — OTA reciprocity self-calibration
`10-ota-reciprocity`
Q: When does over-the-air reciprocity (`Ĥ_AB·Ĥ_BA⁻¹`) cancel hardware and yield a
usable shared reference, and what residual defeats it on ESP32? Must contain:
(a) the identity and what cancels; (b) residual sources — RF-chain asymmetry, LO
drift, reciprocal mixing; (c) cadence vs channel coherence; (d) published residual
magnitudes. Leads: distributed-MIMO / TDD relative-calibration work; Argos; 3GPP
TDD reciprocity. Overlap: not 11, 12.

### 11 — Common-transmitter reference and two-way transfer
`11-common-ref-two-way`
Q: How do a common-transmitter reference and two-way time/frequency/phase transfer
recover a shared reference across independent LOs? Must contain: (a) common-TX
principle and geometry; (b) two-way time transfer and phase transfer; (c) required
SNR/precision; (d) whether a single anchor suffices; (e) the **ping-and-unique-
reply** round-trip scheme: what it can measure and its failure modes. Leads:
IEEE 1588 PTP; White Rabbit (CERN); two-way satellite time transfer;
common-reference distributed MIMO. Overlap: not 10, 12, 17.

### 12 — Consensus, oscillator locking, drift vs update rate
`12-consensus-oscillator`
Q: Can nodes agree on time/frequency via consensus or coupled PLLs, and how does
update rate trade against oscillator drift? Must contain: (a) consensus clock
synchronisation and convergence; (b) PLL injection/locking; (c) ESP32 crystal/TCXO
drift (ppm) and coherence time; (d) update rate vs holdover. Leads: distributed
consensus time sync; Mudumbai (distributed beamforming); ESP32 datasheet clock
specs. Overlap: not 10, 11, 17.

### 13 — Static-multipath and dominant-path self-referencing
`13-static-multipath-self-ref`
Q: Can a static multipath component or dominant LoS path act as a self-reference
to stabilise CSI over time, and how is the static part separated? Must contain:
(a) static/dynamic decomposition; (b) dominant-path/LoS detection; (c) temporal
self-referencing (background subtraction, PCA); (d) stability limits. Leads:
locate static/dynamic CSI decomposition and background-subtraction work. Overlap:
not 04, 14.

### 14 — Map/location-aided calibration and environment-as-antenna
`14-map-aided-env-antenna`
Q: How can a known map or node positions aid calibration, and to what extent can
the environment act as an antenna/aperture? Must contain: (a) map- and
location-aided calibration; (b) environment-as-antenna / reflectarray analogues;
(c) passive-reflector gain; (d) limits of uncontrolled scatter. Leads: map-aided
localisation; reconfigurable-intelligent-surface surveys; ambient backscatter.
Overlap: not 13, 16.

### 15 — Node self-localisation without infrastructure
`15-node-self-localisation`
Q: What accuracy can nodes self-localise from FTM/RTT, RSSI and CSI delay, and
what must be measured? Must contain: (a) FTM/RTT precision and multipath bias;
(b) RSSI vs CSI-delay/time-of-flight; (c) AoA feasibility given ESP32 antenna
count; (d) achievable 2D accuracy and error drivers. Leads: IEEE 802.11mc FTM;
Android Wi-Fi RTT; SpotFi. Overlap: not 02, 16, 17.

### 16 — RF-SLAM, occupancy grids, floorplans, loop closure
`16-rf-slam`
Q: How far can SLAM on RF measurements go — occupancy grids, floorplan inference,
loop closure — and what does it require? Must contain: (a) RF-SLAM formulations;
(b) occupancy-grid/floorplan inference from RF; (c) RF loop closure; (d) sensor
fusion and drift limits. Leads: waveSLAM (arXiv 2312.07192); MOSAIC; Thrun
(Probabilistic Robotics). Overlap: not 03, 14, 15.

### 17 — Time and frequency precision on ESP32-S3
`17-time-frequency-precision`
Q: What precision do ESP32-S3 FTM, the microsecond RX timestamp and TSF actually
provide, and what coherence does that imply? Must contain: (a) RX-timestamp
resolution/accuracy; (b) TSF behaviour and limits; (c) FTM ranging precision;
(d) resulting relative-LO coherence time and phase error. Leads: ESP-IDF Wi-Fi
FTM/timestamp docs; ESP32-S3 datasheet/TRM; IEEE 802.11 timing. Overlap: not 11,
12, 15.

### 18 — Environmental inference beyond humans
`18-environment-inference`
Q: What non-human states can CSI/I/Q infer — objects/furniture, doors/windows,
materials, water/plants, machinery — and with what confidence? Must contain:
(a) object/furniture change detection; (b) door/window open-closed inference;
(c) material and moisture/liquid sensing; (d) human vs animal vs object
classification; (e) confounders. Overlap: not 01, 04, 19.

### 19 — Model-based vs learning-based routes and supervision
`19-model-vs-learning`
Q: For each capability, which of model-based inference or learned models is
appropriate, and what supervision does each need? Must contain: (a)
capability-by-capability route; (b) label requirements (none/synthetic/weak/full);
(c) generalisation across rooms/devices; (d) ESP32 edge vs host compute. Leads:
physics-based CSI models; deep-learning Wi-Fi sensing surveys; CSI domain
adaptation; ESPARGOS/MM-Fi datasets. Overlap: not 01, 02, 16, 18.

### 20 — Theoretical boundary and minimal added reference
`20-theoretical-boundary`
Q: What is provably unobservable from independent-LO receive-only CSI, and what
minimal added reference crosses the boundary? Must contain: (a) formal
unobservability (unknown LO phase/frequency, gauge freedom); (b) invariants
preserved under that gauge; (c) minimal-reference result and the escapes;
(d) Cramér-Rao bounds on calibration precision. Leads: no-common-clock
phase-unobservability results; CRB for localisation/phase; distributed-coherence
theory. Overlap: not 10, 11, 12, 13.
