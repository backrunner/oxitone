use super::super::{advanced as a, banks, motion::lfo_value};
use super::{OscState, Voice, VoiceContext};

impl Voice {
    pub fn configure_envelopes(&mut self, v: &[f64]) {
        self.amp
            .set_curves(v[a::CURVES], v[a::CURVES + 1], v[a::CURVES + 2]);
        self.fenv
            .set_curves(v[a::CURVES + 3], v[a::CURVES + 4], v[a::CURVES + 5]);
        self.mod_env
            .set_curves(v[a::CURVES + 6], v[a::CURVES + 7], v[a::CURVES + 8]);
        self.mod_env.set_params(
            v[a::ENV],
            v[a::ENV + 1],
            v[a::ENV + 2] as f32,
            v[a::ENV + 3],
        );
    }

    #[inline]
    pub fn matrix(&self, ctx: &VoiceContext<'_>) -> [f32; 11] {
        if !ctx.advanced.enabled {
            return [0.; 11];
        }
        let m = ctx.advanced.macros;
        ctx.advanced.evaluate(&[
            0.,
            lfo_value(ctx.motion.shape, self.motion.phase) as f32,
            lfo_value(ctx.advanced.lfo2_shape, self.lfo2_phase) as f32,
            self.amp.level(),
            self.fenv.level(),
            self.mod_env.level(),
            self.velocity,
            ((self.note as f32 - 60.) / 60.).clamp(-1., 1.),
            self.note_random,
            m[0],
            m[1],
            m[2],
            m[3],
        ])
    }
}

impl OscState {
    #[inline]
    pub fn render_sample(
        &mut self,
        ctx: &VoiceContext<'_>,
        index: usize,
        position: f32,
        frequency: f64,
        warp: f32,
        phase_mod: f64,
    ) -> (f32, f32) {
        let (a, b, blend) = if ctx.advanced.banks[index] == 0 {
            if index == 0 {
                (ctx.table_a, ctx.morph_a, position)
            } else {
                (ctx.table_b, ctx.morph_b, position)
            }
        } else {
            let (a, b, t) = banks::pair(ctx.advanced.banks[index], position);
            (&ctx.tables[a], &ctx.tables[b], t)
        };
        let (mut left, mut right) = (0., 0.);
        for u in 0..self.count {
            let hz = frequency * self.ratios[u];
            let sample = self.readers[u].next_warped(
                a,
                b,
                blend,
                hz,
                ctx.advanced.warp_modes[index],
                warp,
                phase_mod,
            );
            if hz < ctx.sample_rate * 0.49 {
                left += sample * self.gain_l[u];
                right += sample * self.gain_r[u];
            }
        }
        (left, right)
    }
}
