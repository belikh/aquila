# External resources

Indexed external sources used across the spec and research. Machine-readable form: `sources.json`.
Cite by `id`. Licences are as reported by the source and should be re-verified before redistribution.

## Datasets

| id | Resource | Licence | Notes |
| --- | --- | --- | --- |
| `mmfi` | MM-Fi — https://ntu-aiot-lab.github.io/mm-fi | CC BY-NC 4.0 | CSI + RGB/depth/LiDAR/mmWave + pose; RGB on request |
| `mmfi-repo` | https://github.com/ybhbingo/MMFi_dataset | — | Dataset tooling |
| `widar3` | Widar3.0 — http://tns.thss.tsinghua.edu.cn/widar3.0/ | IEEE DataPort open access | Gestures; no pose/vitals |
| `piw3d` | Person-in-WiFi-3D — https://github.com/aiotgroup/Person-in-WiFi-3D-repo | unverified | CSI + camera 3D keypoints |
| `csibench` | CSI-Bench — https://github.com/Jenny-Zhu/CSI-Bench-Real-WiFi-Sensing-Benchmark | MIT (repo); data unverified | Includes breathing; no pose |

## Model weights

| id | Resource | Licence | Notes |
| --- | --- | --- | --- |
| `hf-triplet` | https://huggingface.co/ruvnet/wifi-densepose-pretrained | MIT | Encoder only; no pose; one overnight capture |
| `hf-mmfi-pose` | https://huggingface.co/ruvnet/wifi-densepose-mmfi-pose | CC BY-NC 4.0 | Torso-PCK@20 82.686 in-domain; cross-room ~17.5% |

## Projects & tools

| id | Resource | Licence | Notes |
| --- | --- | --- | --- |
| `esp-csi` | https://github.com/espressif/esp-csi | Apache-2.0 | Espressif CSI examples |
| `espectre` | https://github.com/francescopace/espectre | see repo | On-device motion via ESPHome/MQTT |
| `espargos` | https://espargos.net + https://github.com/ESPARGOS/esp-sdr | see repo | Phase-coherent ESP32 array; RX-only raw I/Q |
| `openmac` | https://github.com/ESPARGOS/esp32-open-mac | see repo | Reverse-engineered Wi-Fi MAC |
| `ruview` | https://github.com/ruvnet/RuView | MIT | Reference system being rebuilt |
| `wasmtime` | https://wasmtime.dev | Apache-2.0 WITH LLVM-exception | WASM host (plugin research) |
| `extism` | https://extism.org | BSD-3-Clause | WASM plugin framework (plugin research) |
| `ort` | https://github.com/pykeio/ort | MIT/Apache-2.0 | ONNX Runtime bindings (recommended runtime) |
| `candle` | https://github.com/huggingface/candle | MIT/Apache-2.0 | Pure-Rust ML (alternative) |
| `burn` | https://github.com/tracel-ai/burn | MIT/Apache-2.0 | Pure-Rust ML (alternative) |
| `leptos` | https://leptos.dev | MIT | Rust→WASM UI (chosen dashboard stack) |
| `dioxus` | https://dioxuslabs.com | MIT/Apache-2.0 | Rust→WASM UI (alternative) |
| `criterion` | https://github.com/bheisler/criterion.rs | MIT/Apache-2.0 | Statistical benchmarks |
| `iai-callgrind` | https://github.com/iai-callgrind/iai-callgrind | MIT/Apache-2.0 | Instruction-count benchmarks (deterministic) |
| `cargo-mutants` | https://github.com/sourcefrog/cargo-mutants | MIT | Mutation testing |
| `proptest` | https://github.com/proptest-rs/proptest | MIT/Apache-2.0 | Property testing |
| `hacs` | https://hacs.xyz | — | Home Assistant Community Store |

## Papers & standards

| id | Resource | Notes |
| --- | --- | --- |
| `mmfi-paper` | MM-Fi, NeurIPS 2023 D&B — arXiv:2305.10345 | Dataset paper |
| `piw3d-paper` | Person-in-WiFi-3D | Camera-supervised WiFi pose |
| `metafi` | MetaFi — arXiv:2208.10414 | Licence unverified |
| `xfi` | X-Fi — arXiv:2410.10167 | Modality-invariant |
| `csibench-paper` | CSI-Bench — arXiv:2505.21866 | Vitals benchmark |
| `densepose-wifi` | "DensePose from WiFi" (CMU, 2023) — **citation to verify** | Origin of the camera-teacher idea |
| `ieee-80211bf` | IEEE 802.11bf (WLAN sensing) | Forward-compat only; no conformant commodity silicon |

## Regulatory

| id | Resource | Notes |
| --- | --- | --- |
| `acma-lipd` | ACMA Low Interference Potential Devices (LIPD) class licence — https://www.acma.gov.au | Australian Wi-Fi/ISM operation |
| `fcc-part15` | FCC Part 15 — https://www.fcc.gov | US equivalent |

## Home Assistant

| id | Resource | Notes |
| --- | --- | --- |
| `ha-arch` | https://developers.home-assistant.io/docs/architecture_index/ | Components vs add-ons |
| `ha-addons` | https://developers.home-assistant.io/docs/add-ons/ | Add-on packaging + ingress |
| `ha-hacs` | https://hacs.xyz/docs/publish/start | Publishing a custom component |
| `ha-update` | https://developers.home-assistant.io/docs/core/entity/update/ | `update` entity (firmware OTA surface) |

## Firmware update (OTA)

| id | Resource | Notes |
| --- | --- | --- |
| `esp-idf-ota` | https://docs.espressif.com/projects/esp-idf/en/latest/esp32s3/api-reference/system/ota.html | ESP-IDF OTA API + dual partition |
| `esp-idf-ota-rollback` | same | Rollback / anti-rollback (anchor to verify) |
