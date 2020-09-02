#![allow(clippy::collapsible_if)]

mod noteskin;
use noteskin::*;

use std::borrow::Cow;
use image::{GenericImageView, GenericImage, RgbaImage};
use thiserror::Error;


fn row_to_note_index(row: usize) -> usize {
    for (i, note_type) in [4, 8, 12, 16, 24, 32, 48, 64, 192].iter().enumerate() {
        if row % (192 / note_type) == 0 {
            return i;
        }
    }
    panic!("This can't happen, last loop iteration should cover everything");
}

/// An ad-hoc error type that fits any string literal
#[derive(Debug, Error)]
pub enum Error {
	#[error("Given pattern is empty")]
	EmptyPattern,
	#[error("Error in the image library")]
	ImageError(#[from] image::ImageError),
	#[error("This keymode is not implemented")]
	KeymodeNotImplemented(u32),
}

/// Parameter `note_imgs`: a slice of 64x64 images, in the following order: 4ths, 8ths, 12ths,
/// 16ths, 24ths, 32nds, 48ths, 64ths, 192nds
fn render_pattern(
	noteskin: &dyn Noteskin,
	pattern: &etterna::Pattern,
	scroll_type: etterna::ScrollDirection,
	interval_num_rows: usize,
) -> Result<RgbaImage, Error> {
	let keymode = pattern.keymode().ok_or(Error::EmptyPattern)?;

	// Create an empty image buffer, big enough to fit all the lanes and arrows
	let width = 64 * keymode;
	let height = 64 * pattern.rows.len();
	let mut buffer = image::ImageBuffer::new(width as u32, height as u32);

	let mut place_note = |note_img: &RgbaImage, x, mut y| {
		// Flip y if downscroll
		if scroll_type == etterna::ScrollDirection::Downscroll {
			y = (buffer.height() / 64) - y - 1;
		}

		buffer.copy_from(note_img, x * 64, y * 64)
			.expect("Note image is too large (shouldn't happen)");
	};

	for lane in 0..keymode {
		place_note(&noteskin.receptor(lane), lane, 0);
	}

	for (i, row) in pattern.rows.iter().enumerate() {
		for &lane in row {
			let note_img = noteskin.note(row_to_note_index(i * interval_num_rows), lane);

			place_note(&note_img, lane, i as u32);
		}
	}
	
	Ok(buffer)
}

pub struct PatternVisualizer {
	dbz: NoteskinLdur,
	delta_note: Noteskin5k,
	sbz: NoteskinLdur,
	dbz_6k: Noteskin6k,
}

impl PatternVisualizer {
	pub fn load() -> Result<Self, Error> {
		Ok(Self {
			dbz: NoteskinLdur::read(
				"noteskin/ldur-notes.png", "noteskin/ldur-receptor.png",
				true,
			)?,
			delta_note: Noteskin5k::read(
				"noteskin/5k-center-notes.png", "noteskin/5k-center-receptor.png",
				"noteskin/5k-corner-notes.png", "noteskin/5k-corner-receptor.png"
			)?,
			sbz: NoteskinLdur::read(
				"noteskin/bar-notes.png", "noteskin/bar-receptor.png",
				false,
			)?,
			dbz_6k: Noteskin6k::read(
				"noteskin/ldur-notes.png", "noteskin/ldur-receptor.png",
			)?,
		})
	}

	pub fn generate(&self,
		pattern_str: &str,
		scroll_type: etterna::ScrollDirection,
		interval_num_rows: usize, // e.g. 16 for 16ths, 48 for 48ths
	) -> Result<Vec<u8>, Error> {
		let mut pattern = etterna::Pattern::parse_taps(pattern_str);

		let noteskin: &dyn Noteskin = match pattern.keymode().ok_or(Error::EmptyPattern)? {
			0..=4 | 8 => &self.dbz,
			5 => &self.delta_note,
			7 | 9 => &self.sbz,
			6 => &self.dbz_6k,
			other => return Err(Error::KeymodeNotImplemented(other)),
		};

		pattern.rows.truncate(100);
		let buffer = render_pattern(noteskin, &pattern, scroll_type, interval_num_rows)?;
		
		let mut output_buffer = Vec::with_capacity(1_000_000); // allocate 1 MB for the img
		image::DynamicImage::ImageRgba8(buffer).write_to(
			&mut output_buffer,
			image::ImageOutputFormat::Png
		)?;

		Ok(output_buffer)
	}
}