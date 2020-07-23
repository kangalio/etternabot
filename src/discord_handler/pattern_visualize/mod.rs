#![allow(clippy::collapsible_if)]

mod noteskin;
use noteskin::*;

use std::borrow::Cow;
use image::{GenericImageView, GenericImage, RgbaImage};
use serde::{Deserialize, Serialize};
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
	#[error("There was an open bracket without a corresponding closing bracket")]
	UnterminatedOpenBracket,
	#[error("Given pattern is empty")]
	EmptyPattern,
	#[error("Error in the image library")]
	ImageError(#[from] image::ImageError),
	#[error("This keymode is not implemented")]
	KeymodeNotImplemented(u32),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub enum ScrollType {
	Upscroll,
	Downscroll,
}

struct Pattern {
	/// Each row is a vector of lane numbers. For example a plain jumptrill would be
	/// `vec![vec![0, 1], vec![2, 3], vec![0, 1], vec![2, 3]...]`
	pub rows: Vec<Vec<u32>>,
}

impl Pattern {
	// Determines the keymode (e.g. 4k/5k/6k/...) by adding 1 to the rightmost lane
	pub fn keymode(&self) -> Result<u32, Error> {
		let keymode = (1 + self.rows.iter().flatten().max()
			.ok_or(Error::EmptyPattern)?)
			.max(4); // clamp to a minimum of 4 because even if the pattern is `2323`, it's still 4k
		Ok(keymode)
	}
}

/// Parameter `note_imgs`: a slice of 64x64 images, in the following order: 4ths, 8ths, 12ths,
/// 16ths, 24ths, 32nds, 48ths, 64ths, 192nds
fn render_pattern(
	noteskin: &dyn Noteskin,
	pattern: &Pattern,
	scroll_type: ScrollType,
	interval_num_rows: usize,
) -> Result<RgbaImage, Error> {
	let keymode = pattern.keymode()?;

	// Create an empty image buffer, big enough to fit all the lanes and arrows
	let width = 64 * keymode;
	let height = 64 * pattern.rows.len();
	let mut buffer = image::ImageBuffer::new(width as u32, height as u32);

	let mut place_note = |note_img: &RgbaImage, x, mut y| {
		// Flip y if downscroll
		if scroll_type == ScrollType::Downscroll {
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

/// Returns the width in bytes of the first character in the string
fn first_char_width(string: &str) -> usize {
	for i in 1.. {
		if string.is_char_boundary(i) {
			return i;
		}
	}
	unreachable!();
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
enum CharToLane {
	Some(u32),
	Invalid,
	Space,
}

impl CharToLane {
	pub fn as_some(self) -> Option<u32> {
		match self {
			Self::Some(lane) => Some(lane),
			_ => None,
		}
	}
}

fn char_to_lane(c: u8) -> CharToLane {
	match c.to_ascii_lowercase() {
		b'0' => CharToLane::Space,
		b'1'..=b'9' => CharToLane::Some((c - b'1') as u32),
		b'l' => CharToLane::Some(0),
		b'd' => CharToLane::Some(1),
		b'u' => CharToLane::Some(2),
		b'r' => CharToLane::Some(3),
		_ => CharToLane::Invalid,
	}
}

fn parse_pattern(mut string: &str) -> Result<Pattern, Error> {
	let mut rows = Vec::new();

	// this parser works by 'popping' characters off the start of the string until the string is empty

	while !string.is_empty() {
		// if the next char is a '[', find the matching ']', read all numbers inbetween, put them into a
		// vector, and finally add that vector to the `rows`
		// if the next char is _not_ a '[' and it's a valid number, push a new row with the an arrow in
		// the lane specified by the number
		if string.starts_with('[') {
			let end = string.find(']')
				.ok_or(Error::UnterminatedOpenBracket)?;
			
			rows.push(string[1..end].bytes()
				.filter_map(|c| char_to_lane(c).as_some())
				.collect::<Vec<_>>());
	
			string = &string[end+1..];
		} else {
			match char_to_lane(string.as_bytes()[0]) {
				CharToLane::Some(lane) => rows.push(vec![lane]),
				CharToLane::Space => rows.push(vec![]),
				CharToLane::Invalid => {},
			}

			string = &string[first_char_width(string)..];
		}
	}

	Ok(Pattern { rows })
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
		scroll_type: ScrollType,
		interval_num_rows: usize, // e.g. 16 for 16ths, 48 for 48ths
	) -> Result<Vec<u8>, Error> {
		let mut pattern = parse_pattern(pattern_str)?;

		let noteskin: &dyn Noteskin = match pattern.keymode()? {
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