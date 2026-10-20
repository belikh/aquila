//! Property and golden tests for `aquila-signal`.
//!
//! The properties are the consolidated invariants of `signal.md` §6, plus a
//! determinism test that runs [`process`] twice and asserts bit-identical
//! output.

use approx::assert_relative_eq;
use proptest::prelude::*;

use crate::*;

// --- helpers ---------------------------------------------------------------

fn meta(node: u16, seq: u32, t: u64) -> RxMeta {
    RxMeta {
        node: NodeId(node),
        seq,
        t_device_us: t,
        centre_freq_hz: 2_437_000_000,
        bandwidth: Bandwidth::Ht20,
        rssi_dbm: -45,
        noise_floor_dbm: -95,
        first_word_invalid: false,
    }
}

fn ht20_bins(seed: u32) -> Vec<Complex32> {
    (0..64)
        .map(|i| {
            let re = ((i % 7) as f32 - 3.0) * 0.1 + (seed as f32) * 0.01;
            let im = ((i % 5) as f32 - 2.0) * 0.1 - (seed as f32) * 0.005;
            Complex32::new(re, im)
        })
        .collect()
}

fn synthetic_chunk(frames: u32) -> Vec<ComplexFrame> {
    (0..frames)
        .map(|k| ComplexFrame {
            meta: meta(1, k, u64::from(k) * 50_000),
            bins: ht20_bins(k),
            flags: FrameFlags::default(),
        })
        .collect()
}

fn wrap(x: f32) -> f32 {
    let tau = core::f32::consts::TAU;
    x - tau * (x / tau).round()
}

// --- decode (D) ------------------------------------------------------------

#[test]
fn decode_d1_structural_accept_rule() {
    let layout = SubcarrierLayout::ht20();
    for len in 0..=200usize {
        let raw = vec![0u8; len];
        let result = decode_record(&raw, &meta(1, 0, 0), &layout);
        if len == 128 {
            assert!(result.is_ok(), "len {len} should decode");
        } else {
            let err = result.expect_err("should reject").clone();
            let expected = if len == 0 {
                DecodeError::EmptyRecord
            } else if len > 1428 {
                DecodeError::TooLarge(len)
            } else if len % 2 == 1 {
                DecodeError::OddLength(len)
            } else {
                DecodeError::WrongBinCount {
                    found: len / 2,
                    expected: 64,
                }
            };
            assert_eq!(err, expected, "len {len}");
        }
    }
}

#[test]
fn decode_d3_exact_int8_conversion() {
    let layout = SubcarrierLayout::ht20();
    let mut raw = vec![0u8; 128];
    raw[0] = 5; // imaginary first
    raw[1] = 250; // -6 as i8, real
    let frame = decode_record(&raw, &meta(1, 0, 0), &layout).unwrap();
    assert_eq!(frame.bins[0], Complex32::new(-6.0, 5.0));
}

#[test]
fn decode_d4_first_word_invalid_zeroes_only_first_pair() {
    let layout = SubcarrierLayout::ht20();
    let raw: Vec<u8> = (0..128).map(|i| (i % 200) as u8).collect();
    let mut m = meta(1, 0, 0);
    let clean = decode_record(&raw, &m, &layout).unwrap();
    m.first_word_invalid = true;
    let zeroed = decode_record(&raw, &m, &layout).unwrap();
    assert_eq!(zeroed.bins[0], Complex32::ZERO);
    assert!(zeroed.flags.first_word_zeroed);
    assert_eq!(zeroed.bins[1..], clean.bins[1..]);
}

#[test]
fn decode_d5_metadata_anomalies_do_not_change_bins() {
    let layout = SubcarrierLayout::ht20();
    let raw: Vec<u8> = (0..128).map(|i| (i % 200) as u8).collect();
    let mut m = meta(1, 0, 0);
    let baseline = decode_record(&raw, &m, &layout).unwrap();
    m.rssi_dbm = 5; // out of range
    m.noise_floor_dbm = 10; // out of range
    let flagged = decode_record(&raw, &m, &layout).unwrap();
    assert_eq!(flagged.bins, baseline.bins);
    assert!(flagged.flags.rssi_anomaly);
    assert!(flagged.flags.noise_floor_anomaly);
}

#[test]
fn decode_csi_bridges_the_contract_frame() {
    use aquila_types::{Band, PhyFlags};
    let frame = aquila_types::CsiFrame {
        node_id: NodeId(7),
        boot_epoch: 1,
        sequence: 5,
        device_time_us: 1_000,
        band: Band::B2G4,
        channel: 6,
        bandwidth_mhz: 20,
        subcarrier_offset: 1,
        // Imaginary-first interleaved sample: (Q, I) pairs. Subcarrier 0 is
        // (Q=0, I=64) and subcarrier 1 is (Q=64, I=0).
        i_q: vec![0, 64, 64, 0],
        rssi_dbm: -50,
        noise_floor_dbm: -90,
        phy: PhyFlags::empty(),
        first_word_invalid: false,
        source_mac: None,
        rx_seq: None,
    };
    let decoded = decode_csi(&frame).expect("a valid contract frame decodes");
    assert_eq!(decoded.meta.node, NodeId(7));
    assert_eq!(decoded.meta.seq, 5);
    assert_eq!(decoded.meta.bandwidth, Bandwidth::Ht20);
    assert_eq!(decoded.bins.len(), 2);
    // The imaginary-first order maps the first byte to Q and the second to I.
    assert!((decoded.bins[0].re - 1.0).abs() < 1e-4);
    assert!(decoded.bins[0].im.abs() < 1e-4);
    assert!(decoded.bins[1].re.abs() < 1e-4);
    assert!((decoded.bins[1].im - 1.0).abs() < 1e-4);
    assert!(!decoded.flags.first_word_zeroed);
}

#[test]
fn decode_csi_zeroes_first_pair_when_flagged() {
    use aquila_types::{Band, PhyFlags};
    let mut frame = aquila_types::CsiFrame {
        node_id: NodeId(1),
        boot_epoch: 0,
        sequence: 0,
        device_time_us: 0,
        band: Band::B2G4,
        channel: 6,
        bandwidth_mhz: 20,
        subcarrier_offset: 0,
        i_q: vec![10, 20, 30, 40],
        rssi_dbm: -50,
        noise_floor_dbm: -90,
        phy: PhyFlags::empty(),
        first_word_invalid: false,
        source_mac: None,
        rx_seq: None,
    };
    let clean = decode_csi(&frame).expect("clean decodes");
    assert_eq!(clean.bins.len(), 2);
    frame.first_word_invalid = true;
    let zeroed = decode_csi(&frame).expect("flagged decodes");
    assert_eq!(zeroed.bins[0], Complex32::ZERO);
    assert!(zeroed.meta.first_word_invalid);
    assert!(zeroed.flags.first_word_zeroed);
    assert_eq!(zeroed.bins[1..], clean.bins[1..]);
}

#[test]
fn decode_csi_rejects_malformed_contract_frames() {
    use aquila_types::{Band, PhyFlags};
    let mut frame = aquila_types::CsiFrame {
        node_id: NodeId(1),
        boot_epoch: 0,
        sequence: 0,
        device_time_us: 0,
        band: Band::Unknown,
        channel: 0,
        bandwidth_mhz: 20,
        subcarrier_offset: 0,
        i_q: vec![1, 1, 1],
        rssi_dbm: -50,
        noise_floor_dbm: -90,
        phy: PhyFlags::empty(),
        first_word_invalid: false,
        source_mac: None,
        rx_seq: None,
    };
    // An odd-length interleaved sample is not whole I/Q pairs.
    assert!(matches!(
        decode_csi(&frame),
        Err(DecodeError::OddIqLength(3))
    ));
    frame.i_q.clear();
    assert!(matches!(decode_csi(&frame), Err(DecodeError::EmptyFrame)));
}

// --- unwrap (U) ------------------------------------------------------------

#[test]
fn unwrap_u2_identity_below_pi() {
    let mut phi = vec![-1.0_f32];
    let mut state = -1.0_f32;
    for k in 1..64 {
        state += if k % 2 == 0 { 0.3 } else { -0.25 };
        phi.push(state);
    }
    assert!(!unwrap_boundary(&phi));
    assert_eq!(unwrap_phase(&phi), phi);
}

#[test]
fn unwrap_u3_first_element_preserved() {
    let phi = vec![0.7_f32, -3.0, 3.0, -0.2, 0.1];
    assert_eq!(unwrap_phase(&phi)[0], 0.7);
}

#[test]
fn unwrap_empty_is_empty() {
    assert!(unwrap_phase(&[]).is_empty());
}

proptest! {
    #[test]
    fn unwrap_u1_congruence(phi in prop::collection::vec(-6.0f32..=6.0, 1..64)) {
        let u = unwrap_phase(&phi);
        prop_assert_eq!(u.len(), phi.len());
        for (a, b) in phi.iter().zip(u.iter()) {
            prop_assert!(wrap(a - b).abs() < 1e-3);
        }
    }
}

// --- sanitise (P) ----------------------------------------------------------

#[test]
fn sanitise_p1_affine_recovers_slope() {
    let layout = SubcarrierLayout::ht20();
    let alpha = 0.05_f32;
    let beta = 0.3_f32;
    let u: Vec<f32> = layout
        .usable
        .iter()
        .map(|&k| alpha * k as f32 + beta)
        .collect();
    let out = sanitise_phase(&u, &layout);
    assert!(out.fit_ok);
    assert_relative_eq!(out.slope, alpha, epsilon = 1e-3);
    for r in &out.residual {
        assert!(r.abs() < 1e-2, "residual {r} not near zero");
    }
}

#[test]
fn sanitise_p3_short_input_is_flagged() {
    let layout = SubcarrierLayout::ht20();
    let u = vec![0.1_f32];
    let out = sanitise_phase(&u, &layout);
    assert!(!out.fit_ok);
    assert_eq!(out.residual, u);
}

#[test]
fn sanitise_p6_nonfinite_is_guarded() {
    let layout = SubcarrierLayout::ht20();
    let u = vec![0.0_f32, f32::NAN, 0.0];
    let out = sanitise_phase(&u, &layout);
    assert!(!out.fit_ok);
    assert!(out.residual.iter().any(|v| v.is_nan()));
}

proptest! {
    #[test]
    fn sanitise_p2_orthogonal(u in prop::collection::vec(-3.0f32..3.0, 8..64)) {
        let layout = SubcarrierLayout { expected_bins: 64, usable: (0..u.len() as u16).collect() };
        let out = sanitise_phase(&u, &layout);
        let sum: f32 = out.residual.iter().sum();
        let dot: f32 = out.residual.iter().enumerate().map(|(i, r)| r * i as f32).sum();
        prop_assert!(sum.abs() < 1e-2, "mean {sum}");
        prop_assert!(dot.abs() < 1e-2, "dot {dot}");
    }

    #[test]
    fn sanitise_p5_shift_covariance(u in prop::collection::vec(-2.0f32..2.0, 8..48), c in -5.0f32..5.0) {
        let layout = SubcarrierLayout { expected_bins: 64, usable: (0..u.len() as u16).collect() };
        let base = sanitise_phase(&u, &layout);
        let shifted: Vec<f32> = u.iter().map(|v| v + c).collect();
        let moved = sanitise_phase(&shifted, &layout);
        for (a, b) in base.residual.iter().zip(moved.residual.iter()) {
            prop_assert!((a - b).abs() < 1e-2);
        }
    }
}

// --- normalise (N) ---------------------------------------------------------

#[test]
fn normalise_n1_identity_is_bit_exact() {
    let frame = ComplexFrame {
        meta: meta(1, 0, 0),
        bins: ht20_bins(3),
        flags: FrameFlags::default(),
    };
    let cal = Calibration {
        per_node: vec![Complex32::ONE; 64],
        state: CalibrationState::Calibrated,
    };
    let out = normalise(&frame, &cal);
    for (a, b) in out.bins.iter().zip(frame.bins.iter()) {
        assert_eq!(a.to_bits_pair(), b.to_bits_pair());
    }

    let empty = Calibration {
        per_node: Vec::new(),
        state: CalibrationState::Uncalibrated,
    };
    let out = normalise(&frame, &empty);
    assert_eq!(out.bins, frame.bins);
}

/// A tiny helper so `-0.0` and `0.0` are distinguished in the N1 assertion.
trait BitsPair {
    fn to_bits_pair(self) -> (u32, u32);
}

impl BitsPair for Complex32 {
    fn to_bits_pair(self) -> (u32, u32) {
        (self.re.to_bits(), self.im.to_bits())
    }
}

// --- filter (F) ------------------------------------------------------------

fn motion_coeffs() -> BiquadCoeffs {
    BiquadCoeffs::bandpass(0.1, 5.0, 20.0).unwrap()
}

#[test]
fn bandpass_f4_zero_in_zero_out() {
    let y = bandpass(&vec![0.0_f32; 64], &motion_coeffs());
    assert!(y.iter().all(|v| *v == 0.0));
}

#[test]
fn bandpass_f3_dc_rejection() {
    let y = bandpass(&vec![1.0_f32; 400], &motion_coeffs());
    let tail = &y[300..];
    let peak = tail.iter().fold(0.0_f32, |a, b| a.max(b.abs()));
    assert!(peak < 0.05, "DC leaked: {peak}");
}

#[test]
fn bandpass_f2_linearity() {
    let coeffs = motion_coeffs();
    let x: Vec<f32> = (0..64).map(|i| ((i % 9) as f32 - 4.0) * 0.3).collect();
    let y: Vec<f32> = (0..64).map(|i| ((i % 5) as f32 - 2.0) * 0.2).collect();
    let (alpha, beta) = (0.75_f32, -0.4_f32);
    let combined: Vec<f32> = x
        .iter()
        .zip(y.iter())
        .map(|(a, b)| alpha * a + beta * b)
        .collect();
    let lhs = bandpass(&combined, &coeffs);
    let bx = bandpass(&x, &coeffs);
    let by = bandpass(&y, &coeffs);
    for i in 0..lhs.len() {
        let rhs = alpha * bx[i] + beta * by[i];
        assert_relative_eq!(lhs[i], rhs, epsilon = 1e-4);
    }
}

proptest! {
    #[test]
    fn bandpass_f1_random_bands_stable(lo in 0.05f32..2.0, width in 0.1f32..6.0) {
        let fs = 20.0f32;
        let hi = (lo + width).min(fs / 2.0 - 0.01);
        prop_assume!(hi > lo);
        if let Ok(coeffs) = BiquadCoeffs::bandpass(lo, hi, fs) {
            for section in &coeffs.sections {
                prop_assert!(section.is_stable());
            }
        }
    }
}

// --- motion (M) ------------------------------------------------------------

#[test]
fn motion_m1_translation_invariance() {
    let x = vec![1.0_f32, -0.5, 0.25, 3.0, -2.0, 0.75, 0.1, -0.9];
    let base = motion_features(&x, 4);
    let moved: Vec<f32> = x.iter().map(|v| v + 7.5).collect();
    let shifted = motion_features(&moved, 4);
    for (a, b) in base.iter().zip(shifted.iter()) {
        assert_relative_eq!(a, b, epsilon = 1e-3);
    }
}

#[test]
fn motion_m2_homogeneity_and_m3_nonnegativity() {
    let x = vec![1.0_f32, -0.5, 0.25, 3.0, -2.0, 0.75, 0.1, -0.9];
    let base = motion_features(&x, 4);
    let scaled = motion_features(&x.iter().map(|v| v * 3.0).collect::<Vec<_>>(), 4);
    for (a, b) in base.iter().zip(scaled.iter()) {
        assert_relative_eq!(*b, a * 9.0, epsilon = 1e-2);
        assert!(*a >= 0.0);
    }
    assert_eq!(motion_features(&[2.0; 4], 4), vec![0.0]);
    assert!(motion_features(&x, 0).is_empty());
}

// --- spectrogram (B) -------------------------------------------------------

fn tone(n: usize, bin: usize, amp: f32, window: usize) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let angle = core::f32::consts::TAU * bin as f32 * i as f32 / window as f32;
            amp * angle.cos()
        })
        .collect()
}

#[test]
fn stft_b1_linearity() {
    let cfg = StftConfig {
        window: 8,
        hop: 4,
        fs_hz: 20.0,
    };
    let x: Vec<f32> = (0..16).map(|i| ((i % 7) as f32 - 3.0) * 0.3).collect();
    let y: Vec<f32> = (0..16).map(|i| ((i % 5) as f32 - 2.0) * 0.2).collect();
    let (alpha, beta) = (0.6_f32, -0.35_f32);
    let combined: Vec<f32> = x
        .iter()
        .zip(y.iter())
        .map(|(a, b)| alpha * a + beta * b)
        .collect();
    let lhs = stft(&combined, &cfg);
    let sx = stft(&x, &cfg);
    let sy = stft(&y, &cfg);
    assert_eq!(lhs.frame_count, sx.frame_count);
    for i in 0..lhs.values.len() {
        let rhs_re = alpha * sx.values[i].re + beta * sy.values[i].re;
        let rhs_im = alpha * sx.values[i].im + beta * sy.values[i].im;
        assert_relative_eq!(lhs.values[i].re, rhs_re, epsilon = 1e-3);
        assert_relative_eq!(lhs.values[i].im, rhs_im, epsilon = 1e-3);
    }
}

#[test]
fn stft_b5_monotone_tone_response() {
    let cfg = StftConfig {
        window: 16,
        hop: 16,
        fs_hz: 20.0,
    };
    let small = stft(&tone(16, 4, 0.5, 16), &cfg);
    let large = stft(&tone(16, 4, 2.0, 16), &cfg);
    let a = small.value(0, 4).magnitude();
    let b = large.value(0, 4).magnitude();
    assert!(b > a, "amplitude not monotone: {a} -> {b}");
}

#[test]
fn bvp_track_finds_in_band_bin() {
    let cfg = StftConfig {
        window: 64,
        hop: 64,
        fs_hz: 20.0,
    };
    // A 1.25 Hz tone: bin = 1.25 * 64 / 20 = 4.
    let spec = stft(&tone(64, 4, 1.0, 64), &cfg);
    let track = bvp_track(&spec, Band::new(0.8, 2.5));
    assert_eq!(track.len(), 1);
    assert_relative_eq!(track[0].freq_hz, 1.25, epsilon = 0.7);
}

// --- selection (K) ---------------------------------------------------------

#[test]
fn selection_k1_k2_k3_and_tie_break() {
    let matrix = SubcarrierFeatureMatrix::new(
        vec![1, 2, 3, 4],
        vec![5.0, 5.0, 9.0, 1.0],
        vec![1.0, 1.0, 1.0, 1.0],
    );
    let mask = select_subcarriers(&matrix, 2);
    // Scores: 3 -> 9 (best), then ties at index 1 and 2 -> lowest index 1.
    assert_eq!(mask.selected, vec![1, 3]);
    // Ascending and subset.
    assert!(mask.selected.windows(2).all(|w| w[0] < w[1]));
    // Fewer than k candidates returns all.
    let short = select_subcarriers(&matrix, 10);
    assert_eq!(short.len(), 4);
}

#[test]
fn selection_k6_permutation_invariance() {
    let base = SubcarrierFeatureMatrix::new(
        vec![1, 2, 3, 4, 5],
        vec![1.0, 3.0, 2.0, 5.0, 4.0],
        vec![1.0, 1.0, 1.0, 1.0, 1.0],
    );
    let reversed = SubcarrierFeatureMatrix::new(
        vec![5, 4, 3, 2, 1],
        vec![4.0, 5.0, 2.0, 3.0, 1.0],
        vec![1.0, 1.0, 1.0, 1.0, 1.0],
    );
    assert_eq!(
        select_subcarriers(&base, 3).selected,
        select_subcarriers(&reversed, 3).selected
    );
}

#[test]
fn selection_nonfinite_excluded() {
    let matrix =
        SubcarrierFeatureMatrix::new(vec![1, 2, 3], vec![f32::NAN, 2.0, 3.0], vec![1.0, 1.0, 1.0]);
    let mask = select_subcarriers(&matrix, 3);
    assert!(!mask.contains(1));
}

// --- resample (R) ----------------------------------------------------------

#[test]
fn resample_r2_identity_on_uniform_grid() {
    let x = vec![0.0_f32, 1.0, 2.0, 3.0, 4.0];
    let t = vec![0_u64, 1000, 2000, 3000, 4000];
    let out = resample_uniform(&x, &t, 1000.0).unwrap();
    assert_eq!(out, x);
}

#[test]
fn resample_r3_large_gap_is_irregular() {
    let x = vec![0.0_f32, 1.0, 2.0, 3.0];
    let t = vec![0_u64, 1000, 2_000_000, 2_001_000];
    assert!(matches!(
        resample_uniform(&x, &t, 1000.0),
        Err(ResampleError::TooIrregular { .. })
    ));
}

#[test]
fn resample_r4_non_monotone_rejected() {
    let x = vec![0.0_f32, 1.0];
    let t = vec![1000_u64, 0];
    assert!(matches!(
        resample_uniform(&x, &t, 1000.0),
        Err(ResampleError::NonMonotone(1))
    ));
}

// --- sequence (S) ----------------------------------------------------------

#[test]
fn sequence_s2_contiguous_has_no_anomalies() {
    let frames = synthetic_chunk(6);
    assert!(validate_sequence(&frames).is_empty());
}

#[test]
fn sequence_s3_missing_middle_is_one_gap() {
    let mut frames = synthetic_chunk(6);
    // Remove the middle frame: sequence 0,1,3,4,5.
    frames.remove(2);
    let anomalies = validate_sequence(&frames);
    assert_eq!(anomalies.len(), 1);
    assert_eq!(anomalies[0], SeqAnomaly::Gap { from: 1, to: 3 });
}

#[test]
fn sequence_empty_is_empty() {
    assert!(validate_sequence(&[]).is_empty());
}

// --- vitals ----------------------------------------------------------------

/// A deterministic uniform draw in `[-0.5, 0.5)` from a splitmix-style step.
fn noise(seed: &mut u64) -> f32 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((*seed >> 40) as f32 / (1u64 << 24) as f32) - 0.5
}

/// A sampled sinusoid at `rate_hz` plus deterministic noise.
fn tone_with_noise(n: usize, fs: f32, rate_hz: f32, amplitude: f32) -> Vec<f32> {
    let mut seed = 0x1234_5678_u64;
    (0..n)
        .map(|i| {
            let t = i as f32 / fs;
            amplitude * (core::f32::consts::TAU * rate_hz * t).sin() + 0.05 * noise(&mut seed)
        })
        .collect()
}

#[test]
fn vitals_estimates_a_known_breathing_rate() {
    let cfg = Config::default_reference(SubcarrierLayout::ht20());
    let series = tone_with_noise(20 * 20, 20.0, 0.25, 1.0);
    let estimate = estimate_vitals(&series, &cfg.vitals_config()).breathing;
    assert!(estimate.is_ok(), "status {:?}", estimate.status);
    let brpm = estimate.rate_per_min;
    assert!(
        (brpm - 15.0).abs() <= 2.0,
        "breathing {brpm} brpm is outside 2 brpm of 15"
    );
    assert!(estimate.confidence > 0.0 && estimate.confidence <= 1.0);
}

#[test]
fn vitals_estimates_a_known_heart_rate() {
    let cfg = Config::default_reference(SubcarrierLayout::ht20());
    let series = tone_with_noise(20 * 30, 20.0, 1.2, 1.0);
    let estimate = estimate_vitals(&series, &cfg.vitals_config()).heart;
    assert!(estimate.is_ok(), "status {:?}", estimate.status);
    let bpm = estimate.rate_per_min;
    assert!(
        (bpm - 72.0).abs() <= 6.0,
        "heart {bpm} bpm is outside 6 bpm of 72"
    );
}

#[test]
fn vitals_is_deterministic() {
    let cfg = Config::default_reference(SubcarrierLayout::ht20());
    let series = tone_with_noise(400, 20.0, 0.3, 0.8);
    let first = estimate_vitals(&series, &cfg.vitals_config());
    let second = estimate_vitals(&series, &cfg.vitals_config());
    assert_eq!(first, second);
}

#[test]
fn vitals_claims_nothing_on_a_short_series() {
    let cfg = Config::default_reference(SubcarrierLayout::ht20());
    let series = vec![0.0_f32; 3];
    let estimates = estimate_vitals(&series, &cfg.vitals_config());
    assert!(!estimates.breathing.is_ok());
    assert!(!estimates.heart.is_ok());
    assert_eq!(estimates.breathing.rate_hz, 0.0);
    assert_eq!(estimates.breathing.confidence, 0.0);
    assert_eq!(
        estimates.breathing.status,
        aquila_types::VitalsStatus::InsufficientSignal
    );
}

#[test]
fn vitals_claims_nothing_on_silence() {
    let cfg = Config::default_reference(SubcarrierLayout::ht20());
    let series = vec![0.0_f32; 400];
    let estimates = estimate_vitals(&series, &cfg.vitals_config());
    assert!(!estimates.breathing.is_ok());
    assert!(!estimates.heart.is_ok());
}

// --- process determinism ---------------------------------------------------

#[test]
fn process_is_bit_identical_across_runs() {
    let cfg = Config::default_reference(SubcarrierLayout::ht20());
    let cal = Calibration {
        per_node: vec![Complex32::ONE; 64],
        state: CalibrationState::Calibrated,
    };
    let chunk = synthetic_chunk(6);
    let first = process(&chunk, &cal, &cfg).unwrap();
    let second = process(&chunk, &cal, &cfg).unwrap();
    assert_eq!(first.fingerprint(), second.fingerprint());
    assert_eq!(first, second);
}

#[test]
fn process_packs_the_canonical_feature_vector() {
    let cfg = Config::default_reference(SubcarrierLayout::ht20());
    let cal = Calibration {
        per_node: vec![Complex32::ONE; 64],
        state: CalibrationState::Calibrated,
    };
    let features = process(&synthetic_chunk(6), &cal, &cfg).unwrap();
    let node = features.nodes[0].node;
    let vector = features
        .feature_vector(node)
        .expect("the node has a feature vector");

    // The documented layout is amplitude RMS values then motion energies, in
    // usable-subcarrier order.
    let source = &features.nodes[0];
    let mut expected = source.amplitude_rms.clone();
    expected.extend_from_slice(&source.motion_energy);
    assert_eq!(vector.values(), expected.as_slice());
    assert_eq!(
        vector.len(),
        source.amplitude_rms.len() + source.motion_energy.len()
    );
    assert!(!vector.is_empty());
    assert!(features.feature_vector(NodeId(999)).is_none());
}

#[test]
fn process_computes_a_real_noise_floor() {
    let cfg = Config::default_reference(SubcarrierLayout::ht20());
    let cal = Calibration {
        per_node: vec![Complex32::ONE; 64],
        state: CalibrationState::Calibrated,
    };
    // A chunk whose bins alternate sign each frame: pure high-frequency content
    // the motion bandpass removes, so the out-of-band residual is non-zero.
    let chunk: Vec<ComplexFrame> = (0..16u32)
        .map(|k| ComplexFrame {
            meta: meta(1, k, u64::from(k) * 50_000),
            bins: (0..64)
                .map(|b| {
                    let sign = if k % 2 == 0 { 1.0 } else { -1.0 };
                    Complex32::new(1.0 + sign * 0.5 * (b as f32 * 0.01), sign * 0.5)
                })
                .collect(),
            flags: FrameFlags::default(),
        })
        .collect();
    let features = process(&chunk, &cal, &cfg).unwrap();
    let node = &features.nodes[0];
    assert_eq!(node.noise_power.len(), node.motion_energy.len());
    assert!(
        node.noise_power.iter().any(|p| *p > 0.0),
        "the per-subcarrier noise floor must be computed from the data, not hardcoded zero (S6)"
    );
}

#[test]
fn process_empty_chunk_errors() {
    let cfg = Config::default_reference(SubcarrierLayout::ht20());
    let cal = Calibration::default();
    assert!(matches!(
        process(&[], &cal, &cfg),
        Err(SignalError::EmptyChunk)
    ));
}

#[test]
fn process_calibration_shape_mismatch_errors() {
    let cfg = Config::default_reference(SubcarrierLayout::ht20());
    let cal = Calibration {
        per_node: vec![Complex32::ONE; 3],
        state: CalibrationState::Calibrated,
    };
    assert!(matches!(
        process(&synthetic_chunk(4), &cal, &cfg),
        Err(SignalError::CalibrationShape { .. })
    ));
}

#[test]
fn config_default_is_valid() {
    assert!(Config::default_reference(SubcarrierLayout::ht20())
        .validate()
        .is_ok());
    assert!(SubcarrierLayout::ht20().validate().is_ok());
    assert!(SubcarrierLayout::ht40().validate().is_ok());
}
