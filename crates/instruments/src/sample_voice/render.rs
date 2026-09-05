//! SampleVoice rendering: the RT path (render/pump/feed helpers).

use oxitone_dsp::gain_pan::equal_power_gains;

use super::SampleVoice;

impl SampleVoice {
    /// Render `out_l.len()` frames, accumulating into the outputs.
    /// `chans` is 1 or 2 prepared-sample channels; `zeros` is a
    /// `max_block`-sized zero buffer shared by the instance. RT-safe.
    pub fn render(
        &mut self,
        chans: &[&[f32]],
        zeros: &[f32],
        out_l: &mut [f32],
        out_r: &mut [f32],
    ) {
        let frames = out_l.len();
        while self.discard_remaining > 0 && !self.drained {
            let n = (self.discard_remaining as usize).min(frames);
            let produced = self.pump(chans, zeros, n);
            if produced == 0 {
                break;
            }
            self.discard_remaining -= produced as u64;
        }
        let mut produced = 0;
        if self.discard_remaining == 0 && !self.drained {
            if !self.rate_initialized {
                self.rate_initialized = true;
                self.set_rate(self.target_rate);
            }
            produced = self.pump(chans, zeros, frames);
        }
        let (gl, gr) = equal_power_gains(self.pan);
        let gain = self.gain;
        for i in 0..produced {
            let e = self.amp.next_sample();
            out_l[i] += self.buf_l[i] * e * gain * gl;
            out_r[i] += self.buf_r[i] * e * gain * gr;
        }
        // Keep the envelope advancing through silent frames (startup discard,
        // post-drain gate release) so note-off timing stays sample-accurate.
        for _ in produced..frames {
            self.amp.next_sample();
        }
    }

    /// Produce up to `n` frames into the scratch buffers (`buf_l`/`buf_r`),
    /// feeding preroll zeros, stream frames, or post-stream zeros as needed.
    /// Returns the produced count.
    fn pump(&mut self, chans: &[&[f32]], zeros: &[f32], n: usize) -> usize {
        let ch_r = chans.get(1).copied().unwrap_or(chans[0]);
        let len = n.min(self.buf_l.len());
        let mut produced = 0usize;
        while produced < len && !self.drained {
            let room = len - produced;
            if self.zeros_fed < self.preroll {
                let k = ((self.preroll - self.zeros_fed) as usize).min(room);
                let rl = self.readers[0].process(&zeros[..k], &mut self.buf_l[produced..len]);
                let rr = self.readers[1].process(&zeros[..k], &mut self.buf_r[produced..len]);
                debug_assert_eq!(rl, rr);
                self.zeros_fed += rl.consumed as u64;
                produced += rl.produced;
                if rl.consumed == 0 && rl.produced == 0 {
                    break;
                }
            } else if let Some(k) = self.stream_chunk(room) {
                let (consumed, prod) = if self.reverse {
                    self.fill_reverse(chans[0], k);
                    let rl = self.readers[0]
                        .process(&self.reverse_buf[..k], &mut self.buf_l[produced..len]);
                    self.fill_reverse(ch_r, k);
                    let rr = self.readers[1]
                        .process(&self.reverse_buf[..k], &mut self.buf_r[produced..len]);
                    debug_assert_eq!(rl, rr);
                    (rl.consumed, rl.produced)
                } else {
                    let start = (self.base + self.stream_pos) as usize;
                    let rl = self.readers[0]
                        .process(&chans[0][start..start + k], &mut self.buf_l[produced..len]);
                    let rr = self.readers[1]
                        .process(&ch_r[start..start + k], &mut self.buf_r[produced..len]);
                    debug_assert_eq!(rl, rr);
                    (rl.consumed, rl.produced)
                };
                self.stream_pos += consumed as u64;
                produced += prod;
                if consumed == 0 && prod == 0 {
                    break;
                }
                self.advance_ramp(prod as u32);
                self.check_drained();
            } else {
                // Stream exhausted without a loop: flush the reader window.
                let k = room.min(zeros.len());
                let rl = self.readers[0].process(&zeros[..k], &mut self.buf_l[produced..len]);
                let rr = self.readers[1].process(&zeros[..k], &mut self.buf_r[produced..len]);
                debug_assert_eq!(rl, rr);
                produced += rl.produced;
                if rl.consumed == 0 && rl.produced == 0 {
                    break;
                }
                self.check_drained();
            }
        }
        produced
    }

    /// Frames available from the stream before the loop/end boundary;
    /// wraps the loop first. `None` when the stream is exhausted.
    fn stream_chunk(&mut self, room: usize) -> Option<usize> {
        if let Some((loop_start, loop_end)) = self.loop_region {
            if self.stream_pos >= loop_end {
                self.stream_pos = loop_start;
            }
            let n = (loop_end - self.stream_pos).min(room as u64) as usize;
            return (n > 0).then_some(n);
        }
        if self.stream_pos >= self.stream_len {
            return None;
        }
        Some(((self.stream_len - self.stream_pos).min(room as u64)) as usize)
    }

    fn fill_reverse(&mut self, chan: &[f32], n: usize) {
        let base = self.base as usize;
        let len = self.stream_len as usize;
        let pos = self.stream_pos as usize;
        for (i, slot) in self.reverse_buf[..n].iter_mut().enumerate() {
            *slot = chan[base + len - 1 - (pos + i)];
        }
    }

    fn check_drained(&mut self) {
        if self.loop_region.is_none()
            && self.stream_pos >= self.stream_len
            && self.readers[0].position() >= (self.preroll + self.stream_len) as f64
        {
            self.drained = true;
        }
    }
}
