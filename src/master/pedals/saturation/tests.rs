use super::*;

#[test]
fn reduced_saturation_keeps_integer_impulse_delay_and_parallel_dry_paths() {
    for quality in [Oversampling::FourX, Oversampling::Off] {
        let mut tube = Tube::with_quality(quality);
        tube.set(&settings::Tube {
            mix: 0.0,
            drive: 10.0,
            ..Default::default()
        });
        tube.clear();
        for frame in 0..200 {
            let input = if frame == 0 {
                (0.75, -0.25)
            } else {
                (0.0, 0.0)
            };
            let expected = if frame == Oversampler::LATENCY {
                (0.75, -0.25)
            } else {
                (0.0, 0.0)
            };
            assert_eq!(
                tube.process(input.0, input.1),
                expected,
                "{quality:?} {frame}"
            );
        }
    }
    let mut saturation = Saturation::new(Oversampling::Off);
    for frame in 0..200 {
        let output = saturation.process(if frame == 0 { 0.8 } else { 0.0 }, |x| x * 0.5);
        assert_eq!(
            output,
            if frame == Oversampler::LATENCY {
                0.4
            } else {
                0.0
            }
        );
    }
}
