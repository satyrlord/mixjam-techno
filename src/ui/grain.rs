//! Seeded noise for the painted materials: the pedals' brushed aluminium and
//! the Vintage Mixer's ivory and graphite. Each recipe gives one value for
//! each texel of a tile, and the tile comes out the same on every run.

/// A small seeded generator, so the grain and each pedal's scratches come out
/// the same on every frame and every run.
pub(super) struct Random(pub(super) u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut mixed = self.0;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^ (mixed >> 31)
    }

    /// Uniform in [0, 1).
    pub(super) fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1_u64 << 24) as f32
    }

    /// Standard normal.
    pub(super) fn normal(&mut self) -> f32 {
        let radius = (-2.0 * (1.0 - self.unit()).ln()).sqrt();
        radius * (std::f32::consts::TAU * self.unit()).cos()
    }
}

/// A `width` by `height` tile of noise from `seed`, in rows from the top.
/// `texel` gives each value from the generator and its column, row by row
/// and left to right, so a recipe can draw per-row values at column 0. The
/// tile is scaled to a root mean square of 1.
pub(super) fn field(
    [width, height]: [usize; 2],
    seed: u64,
    mut texel: impl FnMut(&mut Random, usize) -> f32,
) -> Vec<f32> {
    let mut random = Random(seed);
    let mut values = Vec::with_capacity(width * height);
    for _ in 0..height {
        for x in 0..width {
            values.push(texel(&mut random, x));
        }
    }
    unit_rms(&values)
}

/// The values scaled to a root mean square of 1.
fn unit_rms(values: &[f32]) -> Vec<f32> {
    if values.is_empty() {
        return Vec::new();
    }
    let deviation =
        (values.iter().map(|value| value * value).sum::<f32>() / values.len() as f32).sqrt();
    if deviation == 0.0 || !deviation.is_finite() {
        return vec![0.0; values.len()];
    }
    values.iter().map(|value| value / deviation).collect()
}

/// Fine, even grain that has no direction: white noise softened by its
/// nearest neighbours, which wrap around the tile edges, so the tile repeats
/// without a seam. The result has a root mean square of 1.
pub(super) fn soft_field(size: [usize; 2], seed: u64) -> Vec<f32> {
    // A centre weight of 3 against 1 on each side gives neighbouring texels
    // a correlation of about one half, as in fine paper or enamel.
    const TAPS: [f32; 3] = [1.0, 3.0, 1.0];
    let [width, height] = size;
    let white = field(size, seed, |random, _| random.normal());
    let blur = |values: &[f32], step: [usize; 2]| -> Vec<f32> {
        (0..width * height)
            .map(|index| {
                let [x, y] = [index % width, index / width];
                TAPS.iter()
                    .enumerate()
                    .map(|(tap, weight)| {
                        let sx = (x + width + tap * step[0] - step[0]) % width;
                        let sy = (y + height + tap * step[1] - step[1]) % height;
                        weight * values[sy * width + sx]
                    })
                    .sum()
            })
            .collect()
    };
    unit_rms(&blur(&blur(&white, [1, 0]), [0, 1]))
}
