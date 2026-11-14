use super::*;

#[test]
fn moving_min_keeps_a_full_window_and_survives_timestamp_wrap() {
    for span in [1, 89, SPAN] {
        for start in [0, u32::MAX - SPAN as u32] {
            for shape in 0..3 {
                let mut queue = MovingMin::<SPAN> {
                    now: start,
                    ..MovingMin::EMPTY
                };
                let values: Vec<f64> = (0..SPAN * 4)
                    .map(|frame| match shape {
                        0 => frame as f64,
                        1 => -(frame as f64),
                        _ => ((frame * 37) % 101) as f64,
                    })
                    .collect();
                for (frame, &value) in values.iter().enumerate() {
                    let first = (frame + 1).saturating_sub(span);
                    let expected = values[first..=frame]
                        .iter()
                        .copied()
                        .fold(f64::INFINITY, f64::min);
                    assert_eq!(
                        queue.push(value, span),
                        expected,
                        "span {span}, frame {frame}"
                    );
                    assert!(queue.len <= span);
                }
            }
        }
    }
}
