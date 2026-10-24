//! Brightness, contrast and greyscale. The tone is a table of 256 values, one per input level.

use image::{GrayImage, Luma, RgbImage};
use serde::{Deserialize, Serialize};

use crate::model::Adjust;

/// How many tone settings `remember` keeps.
pub const TONE_MEMORY: usize = 3;

/// The three tone controls of a page, taken together so they can be remembered and put back at once.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ToneSetting {
    /// -100 to 100.
    pub brightness: i32,
    /// -100 to 100.
    pub contrast: i32,
    pub grayscale: bool,
}

impl ToneSetting {
    /// The tone of `adjust`.
    pub fn of(adjust: &Adjust) -> ToneSetting {
        ToneSetting {
            brightness: adjust.brightness,
            contrast: adjust.contrast,
            grayscale: adjust.grayscale,
        }
    }

    /// `adjust` with this tone and everything else unchanged.
    pub fn applied_to(self, adjust: Adjust) -> Adjust {
        Adjust {
            brightness: self.brightness,
            contrast: self.contrast,
            grayscale: self.grayscale,
            ..adjust
        }
    }
}

/// The remembered tones after `tone` was set, newest first and at most `TONE_MEMORY` long.
/// `replacing` is the entry the same run of edits created earlier; it is taken out first, so
/// brightness followed by contrast leaves one entry and not two. The default tone is not stored,
/// and a tone that is already listed keeps its place.
pub fn remember(
    list: &[ToneSetting],
    tone: ToneSetting,
    replacing: Option<ToneSetting>,
) -> Vec<ToneSetting> {
    let mut out: Vec<ToneSetting> = list
        .iter()
        .copied()
        .filter(|t| Some(*t) != replacing)
        .collect();
    if tone != ToneSetting::default() && !out.contains(&tone) {
        out.insert(0, tone);
        out.truncate(TONE_MEMORY);
    }
    out
}

/// The output level for every input level, from brightness and contrast (each -100 to 100).
/// Brightness is a gamma curve: 0 and 255 stay where they are, and the middle moves most.
/// Contrast then stretches the levels about mid-grey.
pub fn lut(brightness: i32, contrast: i32) -> [u8; 256] {
    // +100 gives gamma 1/3, -100 gives gamma 3, 0 gives 1.
    let gamma = 3.0f32.powf(-(brightness.clamp(-100, 100) as f32) / 100.0);
    let k = 1.0 + contrast as f32 / 100.0;
    let mut table = [0u8; 256];
    for (v, out) in table.iter_mut().enumerate() {
        let lifted = 255.0 * (v as f32 / 255.0).powf(gamma);
        let x = (lifted - 127.5) * k + 127.5;
        *out = x.round().clamp(0.0, 255.0) as u8;
    }
    table
}

/// Applies the table to every channel of every pixel.
pub fn apply(img: &mut RgbImage, table: &[u8; 256]) {
    for pixel in img.pixels_mut() {
        for channel in pixel.0.iter_mut() {
            *channel = lookup(table, *channel);
        }
    }
}

/// The table entry for `v`. Every u8 is a valid index, so the fallback is never used.
pub(crate) fn lookup(table: &[u8; 256], v: u8) -> u8 {
    table.get(usize::from(v)).copied().unwrap_or(v)
}

/// The greyscale picture, with luma = round(0.299 R + 0.587 G + 0.114 B).
pub fn to_gray(img: &RgbImage) -> GrayImage {
    let mut gray = GrayImage::new(img.width(), img.height());
    for (src, dst) in img.pixels().zip(gray.pixels_mut()) {
        let [r, g, b] = src.0;
        let luma = 0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b);
        *dst = Luma([luma.round().clamp(0.0, 255.0) as u8]);
    }
    gray
}

/// `remember`, and the entry the run of edits owns afterwards: `tone` when this call put it in the
/// list, else none, because a tone that was already listed is not the run's to replace.
pub fn record(
    list: &[ToneSetting],
    run: Option<ToneSetting>,
    tone: ToneSetting,
) -> (Vec<ToneSetting>, Option<ToneSetting>) {
    let out = remember(list, tone, run);
    let owned = (out.contains(&tone) && !list.contains(&tone)).then_some(tone);
    (out, owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn tone(brightness: i32, contrast: i32) -> ToneSetting {
        ToneSetting {
            brightness,
            contrast,
            grayscale: false,
        }
    }

    #[test]
    fn new_tone_goes_to_the_front() {
        let list = remember(&[tone(10, 0)], tone(20, 5), None);
        assert_eq!(list, vec![tone(20, 5), tone(10, 0)]);
    }

    #[test]
    fn list_is_cut_to_three() {
        let list = [tone(1, 0), tone(2, 0), tone(3, 0)];
        let list = remember(&list, tone(4, 0), None);
        assert_eq!(list, vec![tone(4, 0), tone(1, 0), tone(2, 0)]);
    }

    #[test]
    fn known_tone_keeps_its_place() {
        let list = [tone(1, 0), tone(2, 0), tone(3, 0)];
        assert_eq!(remember(&list, tone(3, 0), None), list.to_vec());
    }

    #[test]
    fn default_tone_is_not_stored() {
        let list = [tone(1, 0)];
        assert_eq!(remember(&list, ToneSetting::default(), None), list.to_vec());
    }

    #[test]
    fn replacing_entry_is_swapped_for_the_new_one() {
        let list = [tone(30, 0), tone(7, 7)];
        let list = remember(&list, tone(30, 20), Some(tone(30, 0)));
        assert_eq!(list, vec![tone(30, 20), tone(7, 7)]);
    }

    #[test]
    fn replacing_an_unlisted_tone_changes_nothing_else() {
        let list = [tone(7, 7)];
        let list = remember(&list, tone(30, 20), Some(tone(99, 99)));
        assert_eq!(list, vec![tone(30, 20), tone(7, 7)]);
    }

    #[test]
    fn two_edits_of_a_run_leave_one_entry() {
        let (list, run) = record(&[], None, tone(30, 0));
        assert_eq!((list.clone(), run), (vec![tone(30, 0)], Some(tone(30, 0))));
        let (list, run) = record(&list, run, tone(30, 20));
        assert_eq!((list, run), (vec![tone(30, 20)], Some(tone(30, 20))));
    }

    #[test]
    fn a_new_run_adds_a_second_entry() {
        let (list, _) = record(&[], None, tone(30, 0));
        let (list, run) = record(&list, None, tone(30, 20));
        assert_eq!(list, vec![tone(30, 20), tone(30, 0)]);
        assert_eq!(run, Some(tone(30, 20)));
    }

    #[test]
    fn a_listed_tone_is_not_owned_by_the_run() {
        let list = [tone(5, 5), tone(7, 7)];
        let (out, run) = record(&list, None, tone(7, 7));
        assert_eq!((out, run), (list.to_vec(), None));
    }

    #[test]
    fn the_default_tone_ends_the_ownership() {
        let (out, run) = record(&[tone(30, 0)], Some(tone(30, 0)), ToneSetting::default());
        assert_eq!((out, run), (vec![], None));
    }

    #[test]
    fn of_and_applied_to_round_trip() {
        let page = Adjust {
            quarter_turns: 1,
            angle_deg: 2.5,
            alternate: true,
            brightness: 40,
            contrast: -10,
            grayscale: true,
            ..Adjust::default()
        };
        let other = Adjust {
            brightness: 5,
            ..Adjust::default()
        };
        let moved = ToneSetting::of(&page).applied_to(other);
        assert_eq!(ToneSetting::of(&moved), ToneSetting::of(&page));
        assert_eq!(
            moved,
            Adjust {
                brightness: 40,
                contrast: -10,
                grayscale: true,
                ..other
            }
        );
    }

    #[test]
    fn zero_zero_is_identity_table() {
        let table = lut(0, 0);
        for v in 0..=255u8 {
            assert_eq!(table[usize::from(v)], v);
        }
    }

    #[test]
    fn brightness_keeps_black_and_white() {
        for b in [-100, -50, 50, 100] {
            let table = lut(b, 0);
            assert_eq!(table[0], 0, "black at brightness {b}");
            assert_eq!(table[255], 255, "white at brightness {b}");
        }
    }

    #[test]
    fn brightness_100_lifts_mid_grey_to_203() {
        assert_eq!(lut(100, 0)[128], 203);
    }

    #[test]
    fn brightness_minus_100_darkens_mid_grey_to_32() {
        assert_eq!(lut(-100, 0)[128], 32);
    }

    #[test]
    fn brightness_lifts_ink_less_than_before() {
        assert_eq!(lut(50, 0)[30], 74);
    }

    #[test]
    fn brightness_table_is_monotonic() {
        for b in [80, -80] {
            let table = lut(b, 0);
            assert!(table.windows(2).all(|w| w[0] <= w[1]), "brightness {b}");
        }
    }

    #[test]
    fn contrast_minus_100_flattens_to_mid_grey() {
        for v in lut(0, -100) {
            assert!(v == 127 || v == 128, "entry {v}");
        }
    }

    #[test]
    fn table_is_monotonic_for_positive_contrast() {
        let table = lut(0, 50);
        assert!(table.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn gray_of_pure_green_is_150() {
        let img = RgbImage::from_pixel(1, 1, Rgb([0, 255, 0]));
        assert_eq!(to_gray(&img).get_pixel(0, 0).0, [150]);
    }
}
