#![allow(clippy::collapsible_if)]

use std::borrow::Cow;
use image::{GenericImageView, GenericImage, RgbaImage};
use serde::{Deserialize, Serialize};


/// An ad-hoc error type that fits any string literal
#[derive(Debug)]
pub struct StringError(&'static str);
impl std::fmt::Display for StringError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		self.0.fmt(f)
	}
}
impl std::error::Error for StringError {}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub enum ScrollType {
	Upscroll,
	Downscroll,
}

struct NoteSkin {
	notes: Vec<RgbaImage>,
	receptor: RgbaImage,
}

impl NoteSkin {
	/// Read the given noteskin image path and split it into multiple note images, each of size
	/// 64x64
	pub fn from_files(
		noteskin_path: &str,
		noteskin_receptor_path: &str,
	) -> anyhow::Result<Self> {
		let mut img = image::open(noteskin_path)?;
		assert_eq!(img.width(), 64);
	
		let mut notes = Vec::new();
		for y in (0..img.height()).step_by(64) {
			notes.push(img.crop(0, y, 64, 64).into_rgba());
		}

		let receptor = image::open(noteskin_receptor_path)?.crop(0, 0, 64, 64).into_rgba();

		Ok(Self { notes, receptor })
	}

	pub fn receptor(&self) -> &RgbaImage { &self.receptor }
	pub fn note(&self, index: usize) -> &RgbaImage { &self.notes[index] }
}

struct Pattern {
	/// Each row is a vector of lane numbers. For example a plain jumptrill would be
	/// `vec![vec![0, 1], vec![2, 3], vec![0, 1], vec![2, 3]...]`
	pub rows: Vec<Vec<u32>>,
}

/// Parameter `note_imgs`: a slice of 64x64 images, in the following order: 4ths, 8ths, 12ths,
/// 16ths, 24ths, 32nds, 48ths, 64ths, 192nds
fn render_pattern(
	noteskin: &NoteSkin,
	pattern: &Pattern,
	scroll_type: ScrollType,
) -> anyhow::Result<RgbaImage> {
	// Determines the keymode (e.g. 4k/5k/6k/...) by adding 1 to the rightmost lane
	let keymode = 1 + *pattern.rows.iter().flatten().max()
		.ok_or(StringError("Given pattern is empty"))?;

	// Create an empty image buffer, big enough to fit all the lanes and arrows
	let width = 64 * keymode;
	let height = 64 * pattern.rows.len();
	let mut buffer = image::ImageBuffer::new(width as u32, height as u32);

	let mut place_note = |note_img: &RgbaImage, x, mut y| {
		// Flip y if downscroll
		if scroll_type == ScrollType::Downscroll {
			y = (buffer.height() / 64) - y - 1;
		}

		// Rotate appropriately
		let note_img = match x {
			0 => Cow::Owned(image::imageops::rotate90(note_img)),
			1 => Cow::Borrowed(note_img),
			2 => Cow::Owned(image::imageops::rotate180(note_img)),
			3 => Cow::Owned(image::imageops::rotate270(note_img)),
			_ => Cow::Borrowed(note_img),
		};

		buffer.copy_from(note_img.as_ref(), x * 64, y * 64)
			.expect("Note image is too large");
	};

	for x in 0..keymode {
		place_note(noteskin.receptor(), x, 0);
	}

	for (i, row) in pattern.rows.iter().enumerate() {
		// Select a note image in the order of 4th-16th-8th-16th (cycle repeats)
		let note_img = noteskin.note([0, 3, 1, 3][i % 4]);

		for &lane in row {
			place_note(note_img, lane, i as u32);
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

fn char_to_lane(c: u8) -> Option<u32> {
	match c {
		b'1'..=b'9' => Some((c - b'1') as u32),
		b'l' | b'L' => Some(0),
		b'd' | b'D' => Some(1),
		b'u' | b'U' => Some(2),
		b'r' | b'R' => Some(3),
		_ => None,
	}
}

fn parse_pattern(mut string: &str) -> anyhow::Result<Pattern> {
	let mut rows = Vec::new();

	// this parser works by 'popping' characters off the start of the string until the string is empty

	while !string.is_empty() {
		// if the next char is a '[', find the matching ']', read all numbers inbetween, put them into a
		// vector, and finally add that vector to the `rows`
		// if the next char is _not_ a '[' and it's a valid number, push a new row with the an arrow in
		// the lane specified by the number
		if string.starts_with('[') {
			let end = string.find(']')
				.ok_or(StringError("Unterminated ["))?;
			
			rows.push(string[1..end].bytes().filter_map(char_to_lane).collect::<Vec<_>>());
	
			string = &string[end+1..];
		} else {
			if let Some(lane) = char_to_lane(string.as_bytes()[0]) {
				rows.push(vec![lane]);
			}

			string = &string[first_char_width(string)..];
		}
	}

	Ok(Pattern { rows })
}

/// Read noteskin from `noteskin_path`, read the pattern from `pattern_str` and write the generated
/// image into `output_path`
pub fn generate(
	output_path: &str,
	pattern_str: &str,
	scroll_type: ScrollType,
) -> anyhow::Result<()> {

	let noteskin = NoteSkin::from_files("noteskin/notes.png", "noteskin/receptor.png")?;
	let mut pattern = parse_pattern(pattern_str)?;
	pattern.rows.truncate(100);
	let buffer = render_pattern(&noteskin, &pattern, scroll_type)?;
	
	buffer.save(output_path)?;

	Ok(())
}