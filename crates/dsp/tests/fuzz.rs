use proptest::prelude::*;
use soundcraft_dsp::{create, offline, plugins};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// Removed upstream processors: any id, any parameter metadata lookup — never instantiable,
    /// never a panic (replaces the retired bundled-plugin fuzz case).
    #[test]
    fn removed_processors_are_never_instantiated(which in 0usize..64, value in any::<f32>()) {
        let info = plugins()[which % plugins().len()];
        prop_assert!(create(info.id).is_none());
        for param in info.params {
            prop_assert!(param.clamp(value).is_finite());
        }
    }

    /// Offline helpers accept any arguments without panicking.
    #[test]
    fn offline_survives_random_args(
        data in proptest::collection::vec(any::<f32>(), 0..400),
        a in any::<f32>(),
        n in 0usize..1000,
        r in any::<f64>(),
    ) {
        let ch = vec![data.clone(), data];
        let _ = offline::non_silent_ranges(&ch, a, n, n, n);
        let _ = offline::detect_transients(&ch, a, a);
        let _ = offline::time_stretch(&ch, r, a);
        let _ = offline::pitch_shift(&ch, a, a);
        let _ = offline::resample(&ch, n as u32, (n as u32).wrapping_mul(3) % 200_000);
        let mut m = ch.clone();
        offline::normalize(&mut m, a, n % 2 == 0);
        offline::fade(&mut m, n, n / 2, offline::FadeShape::SCurve);
        offline::gain(&mut m, a);
        let _ = soundcraft_dsp::spectrum::magnitude_db(&ch[0], n);
    }
}
