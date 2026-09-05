//! Stereo non-interleaved → device-native interleaved layout conversion
//! (03-audio-runtime-spec.md §低延迟和设备: worker 写 ring 前转成设备原生
//! 布局；mono 设备输出 (L+R)*0.5；声道数大于 2 的设备其余声道写 0).
//! Runs on the render worker (buffered mode) or, in direct mode, inside
//! the callback as the final interleave step — allocation-free either way.

/// Convert a stereo block to interleaved device layout. `out` must hold
/// exactly `frames * channels` samples.
pub fn stereo_to_device(left: &[f32], right: &[f32], out: &mut [f32], channels: usize) {
    let frames = left.len().min(right.len());
    debug_assert!(out.len() >= frames * channels);
    match channels {
        1 => {
            for (i, slot) in out[..frames].iter_mut().enumerate() {
                *slot = 0.5 * (left[i] + right[i]);
            }
        }
        2 => {
            for i in 0..frames {
                out[2 * i] = left[i];
                out[2 * i + 1] = right[i];
            }
        }
        n => {
            for i in 0..frames {
                let base = i * n;
                out[base] = left[i];
                out[base + 1] = right[i];
                for slot in &mut out[base + 2..base + n] {
                    *slot = 0.0;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_downmix_is_half_sum() {
        let l = [1.0, -0.5, 0.25];
        let r = [0.5, 0.5, -0.25];
        let mut out = [0.0; 3];
        stereo_to_device(&l, &r, &mut out, 1);
        assert_eq!(out, [0.75, 0.0, 0.0]);
    }

    #[test]
    fn stereo_interleaves() {
        let l = [1.0, 2.0];
        let r = [3.0, 4.0];
        let mut out = [0.0; 4];
        stereo_to_device(&l, &r, &mut out, 2);
        assert_eq!(out, [1.0, 3.0, 2.0, 4.0]);
    }

    #[test]
    fn extra_channels_are_zeroed() {
        let l = [1.0];
        let r = [2.0];
        let mut out = [9.0; 6];
        stereo_to_device(&l, &r, &mut out, 6);
        assert_eq!(out, [1.0, 2.0, 0.0, 0.0, 0.0, 0.0]);
    }
}
