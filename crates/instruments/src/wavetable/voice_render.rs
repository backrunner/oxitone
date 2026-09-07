use super::{Voice, VoiceContext, FILTER_CHUNK};
use oxitone_dsp::biquad::design;

impl Voice {
    /// Render `out_l.len()` frames, accumulating into the stereo output.
    /// RT-safe.
    pub fn render(&mut self, out_l: &mut [f32], out_r: &mut [f32], ctx: &VoiceContext<'_>) {
        let frames = out_l.len();
        let mut i = 0usize;
        while i < frames {
            let chunk = i / FILTER_CHUNK;
            let update =
                !(ctx.motion.enabled || ctx.advanced.enabled) || self.control_remaining == 0;
            if update {
                self.control_remaining = FILTER_CHUNK;
            }
            let end = (i + self.control_remaining).min(frames);
            if update {
                let fenv = self.fenv.level() as f64;
                self.modulation = self.matrix(ctx);
                let modulation = ctx.motion.value(self.motion.phase);
                let cutoff = (ctx.cutoff_chunks[chunk]
                    * 2f64.powf(
                        (ctx.fenv_amount_semis * fenv
                            + ctx.motion.cutoff * modulation
                            + self.modulation[1] as f64 * 48.)
                            / 12.0,
                    ))
                .clamp(10.0, ctx.sample_rate / 2.0 - 1.0);
                let q = 0.5 + 9.5 * ctx.resonance_chunks[chunk];
                let coeffs = design(ctx.filter_kind, ctx.sample_rate, cutoff, q, 0.0);
                self.filter_l.set_coeffs(coeffs);
                self.filter_r.set_coeffs(coeffs);
                let freq = 440.0
                    * 2f64.powf(
                        (self.pitch_semis - 69.0
                            + ctx.motion.pitch * modulation
                            + self.modulation[0] as f64 * 24.)
                            / 12.0,
                    );
                self.render_freq = freq;
                if ctx.motion.pitch != 0. || self.glide_step != 0. || ctx.advanced.enabled {
                    // Conservative spectral headroom for warp/phase modulation. Kept
                    // at control rate; all bank frames share the same mip layout.
                    let warp = |i: usize| {
                        if ctx.advanced.warp_modes[i] == 0 {
                            1.
                        } else {
                            12.
                        }
                    };
                    let fm = if ctx.advanced.fm != 0. || self.modulation[6] != 0. {
                        16.
                    } else {
                        1.
                    };
                    for u in 0..self.osc_a.count {
                        self.osc_a.readers[u]
                            .prepare(ctx.table_a, freq * self.osc_a.ratios[u] * warp(0) * fm);
                    }
                    for u in 0..self.osc_b.count {
                        self.osc_b.readers[u]
                            .prepare(ctx.table_b, freq * self.osc_b.ratios[u] * warp(1));
                    }
                }
                self.sub_reader.prepare(
                    &ctx.tables[super::super::banks::sub_table(ctx.sub_wave)],
                    freq * ctx.motion.sub_ratio,
                );
            }
            let freq = self.render_freq;
            for k in i..end {
                let lfo = ctx.motion.value(self.motion.phase) as f32;
                let modulation = self.matrix(ctx);
                self.motion.phase = (self.motion.phase + ctx.motion.increment).rem_euclid(1.);
                let position_a =
                    (ctx.motion.positions[0] + lfo * ctx.motion.positions_depth[0] + modulation[2])
                        .clamp(0., 1.);
                let position_b =
                    (ctx.motion.positions[1] + lfo * ctx.motion.positions_depth[1] + modulation[3])
                        .clamp(0., 1.);
                let (bl, br) = self.osc_b.render_sample(
                    ctx,
                    1,
                    position_b,
                    freq,
                    (ctx.advanced.warps[1] + modulation[5]).clamp(0., 1.),
                    0.,
                );
                let fm = (ctx.advanced.fm + modulation[6]).clamp(0., 1.);
                let (mut al, mut ar) = self.osc_a.render_sample(
                    ctx,
                    0,
                    position_a,
                    freq,
                    (ctx.advanced.warps[0] + modulation[4]).clamp(0., 1.),
                    (bl + br) as f64 * 0.25 * fm as f64,
                );
                let ring = (ctx.advanced.ring + modulation[7]).clamp(0., 1.);
                al *= 1. - ring + ring * bl;
                ar *= 1. - ring + ring * br;
                let blend = (ctx.mix[k] + modulation[8]).clamp(0., 1.);
                let ma = (1.0 - blend) * ctx.advanced.levels[0];
                let mb = blend * ctx.advanced.levels[1];
                let noise = if ctx.motion.noise_level == 0. {
                    0.
                } else {
                    self.motion.noise() * ctx.motion.noise_level * std::f32::consts::FRAC_1_SQRT_2
                };
                let sub_hz = freq * ctx.motion.sub_ratio;
                let sub = if ctx.motion.sub_level == 0. || sub_hz >= ctx.sample_rate * 0.49 {
                    0.
                } else {
                    let sample = if ctx.sub_wave == 0 {
                        (std::f64::consts::TAU * self.motion.sub_phase).sin() as f32
                    } else {
                        self.sub_reader.set_phase(self.motion.sub_phase);
                        self.sub_reader.next(
                            &ctx.tables[super::super::banks::sub_table(ctx.sub_wave)],
                            sub_hz,
                        )
                    };
                    sample * ctx.motion.sub_level * std::f32::consts::FRAC_1_SQRT_2
                };
                self.motion.sub_phase = (self.motion.sub_phase
                    + freq * ctx.motion.sub_ratio / ctx.sample_rate)
                    .rem_euclid(1.);
                let l = self.filter_l.next(al * ma + bl * mb + noise) + sub;
                let r = self.filter_r.next(ar * ma + br * mb + noise) + sub;
                let e = self.amp.next_sample()
                    * self.velocity
                    * (1. - ctx.motion.level * (1. - lfo) * 0.5)
                    * (1. + modulation[10]).clamp(0., 2.);
                self.fenv.next_sample();
                self.mod_env.next_sample();
                self.lfo2_phase = (self.lfo2_phase + ctx.advanced.lfo2_increment).rem_euclid(1.);
                let pan = modulation[9].clamp(-1., 1.);
                out_l[k] += l * e * (1. - pan.max(0.));
                out_r[k] += r * e * (1. + pan.min(0.));
            }
            self.advance_glide((end - i) as f64);
            self.control_remaining -= end - i;
            i = end;
        }
    }
}
