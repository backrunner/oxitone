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
            let update = !ctx.motion.enabled || self.control_remaining == 0;
            if update {
                self.control_remaining = FILTER_CHUNK;
            }
            let end = (i + self.control_remaining).min(frames);
            if update {
                let fenv = self.fenv.level() as f64;
                let modulation = ctx.motion.value(self.motion.phase);
                let cutoff = (ctx.cutoff_chunks[chunk]
                    * 2f64.powf(
                        (ctx.fenv_amount_semis * fenv + ctx.motion.cutoff * modulation) / 12.0,
                    ))
                .clamp(10.0, ctx.sample_rate / 2.0 - 1.0);
                let q = 0.5 + 9.5 * ctx.resonance_chunks[chunk];
                let coeffs = design(ctx.filter_kind, ctx.sample_rate, cutoff, q, 0.0);
                self.filter_l.set_coeffs(coeffs);
                self.filter_r.set_coeffs(coeffs);
                let freq = 440.0
                    * 2f64.powf((self.pitch_semis - 69.0 + ctx.motion.pitch * modulation) / 12.0);
                self.render_freq = freq;
                if ctx.motion.pitch != 0. || self.glide_step != 0. {
                    for u in 0..self.osc_a.count {
                        self.osc_a.readers[u].prepare(ctx.table_a, freq * self.osc_a.ratios[u]);
                    }
                    for u in 0..self.osc_b.count {
                        self.osc_b.readers[u].prepare(ctx.table_b, freq * self.osc_b.ratios[u]);
                    }
                }
            }
            let freq = self.render_freq;
            for k in i..end {
                let lfo = ctx.motion.value(self.motion.phase) as f32;
                self.motion.phase = (self.motion.phase + ctx.motion.increment).rem_euclid(1.);
                let position_a =
                    (ctx.motion.positions[0] + lfo * ctx.motion.positions_depth[0]).clamp(0., 1.);
                let position_b =
                    (ctx.motion.positions[1] + lfo * ctx.motion.positions_depth[1]).clamp(0., 1.);
                let (mut al, mut ar) = (0.0f32, 0.0f32);
                for u in 0..self.osc_a.count {
                    let v = self.osc_a.readers[u].next_blend(
                        ctx.table_a,
                        ctx.morph_a,
                        position_a,
                        freq * self.osc_a.ratios[u],
                    );
                    al += v * self.osc_a.gain_l[u];
                    ar += v * self.osc_a.gain_r[u];
                }
                let (mut bl, mut br) = (0.0f32, 0.0f32);
                for u in 0..self.osc_b.count {
                    let v = self.osc_b.readers[u].next_blend(
                        ctx.table_b,
                        ctx.morph_b,
                        position_b,
                        freq * self.osc_b.ratios[u],
                    );
                    bl += v * self.osc_b.gain_l[u];
                    br += v * self.osc_b.gain_r[u];
                }
                let mb = ctx.mix[k];
                let ma = 1.0 - mb;
                let noise = if ctx.motion.noise_level == 0. {
                    0.
                } else {
                    self.motion.noise() * ctx.motion.noise_level * std::f32::consts::FRAC_1_SQRT_2
                };
                let sub = if ctx.motion.sub_level == 0. {
                    0.
                } else {
                    (std::f64::consts::TAU * self.motion.sub_phase).sin() as f32
                        * ctx.motion.sub_level
                        * std::f32::consts::FRAC_1_SQRT_2
                };
                self.motion.sub_phase = (self.motion.sub_phase
                    + freq * ctx.motion.sub_ratio / ctx.sample_rate)
                    .rem_euclid(1.);
                let l = self.filter_l.next(al * ma + bl * mb + noise) + sub;
                let r = self.filter_r.next(ar * ma + br * mb + noise) + sub;
                let e = self.amp.next_sample()
                    * self.velocity
                    * (1. - ctx.motion.level * (1. - lfo) * 0.5);
                self.fenv.next_sample();
                out_l[k] += l * e;
                out_r[k] += r * e;
            }
            self.advance_glide((end - i) as f64);
            self.control_remaining -= end - i;
            i = end;
        }
    }
}
