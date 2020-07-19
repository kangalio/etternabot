#![allow(clippy::collapsible_if)]

use std::borrow::Cow;
use image::{GenericImageView, GenericImage, RgbaImage};
use serde::{Deserialize, Serialize};
use thiserror::Error;


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

trait Noteskin {
	fn receptor(&self, lane: u32) -> Cow<RgbaImage>;
	fn note(&self, index: usize, lane: u32) -> Cow<RgbaImage>;
}

struct NoteskinLdur {
	notes: Vec<RgbaImage>,
	receptor: RgbaImage,
	rotate: bool,
}

impl NoteskinLdur {
	/// Read the given noteskin image path and split it into multiple note images, each of size
	/// 64x64
	pub fn read(
		notes_path: &str,
		receptor_path: &str,
		rotate: bool,
	) -> Result<Self, Error> {
		let mut img = image::open(notes_path)?;
	
		let notes: Vec<_> = (0..img.height())
			.step_by(64)
			.map(|y| img.crop(0, y, 64, 64).into_rgba())
			.collect();

		let receptor = image::open(receptor_path)?.crop(0, 0, 64, 64).into_rgba();

		Ok(Self { notes, receptor, rotate })
	}

	fn rotate(img: &RgbaImage, lane: u32, rotate: bool) -> Cow<RgbaImage> {
		if !rotate { return Cow::Borrowed(img) }

		match lane % 4 {
			0 => Cow::Owned(image::imageops::rotate90(img)),
			1 => Cow::Borrowed(img),
			2 => Cow::Owned(image::imageops::rotate180(img)),
			3 => Cow::Owned(image::imageops::rotate270(img)),
			_ => unreachable!(),
		}
	}
}

impl Noteskin for NoteskinLdur {
	fn receptor(&self, lane: u32) -> Cow<RgbaImage> {
		Self::rotate(&self.receptor, lane, self.rotate)
	}

	fn note(&self, index: usize, lane: u32) -> Cow<RgbaImage> {
		Self::rotate(&self.notes[index], lane, self.rotate)
	}
}

struct Noteskin5k {
	center_notes: Vec<RgbaImage>,
	center_receptor: RgbaImage,
	// corner images point left-down
	corner_notes: Vec<RgbaImage>,
	corner_receptor: RgbaImage,
}

impl Noteskin5k {
	pub fn read(
		center_notes_path: &str,
		center_receptor_path: &str,
		corner_notes_path: &str,
		corner_receptor_path: &str,
	) -> Result<Self, Error> {
		let center_receptor = image::open(center_receptor_path)?.crop(0, 0, 64, 64).into_rgba();
		let corner_receptor = image::open(corner_receptor_path)?.crop(0, 0, 64, 64).into_rgba();

		let img = image::open(center_notes_path)?;
		let center_notes: Vec<_> = (0..img.height())
			.step_by(64)
			.map(|y| img.crop_imm(0, y, 64, 64).into_rgba())
			.collect();
		
		let img = image::open(corner_notes_path)?;
		let corner_notes: Vec<_> = (0..img.height())
			.step_by(64)
			.map(|y| img.crop_imm(0, y, 64, 64).into_rgba())
			.collect();

		Ok(Self { center_notes, center_receptor, corner_notes, corner_receptor })
	}

	fn get_img<'a>(corner: &'a RgbaImage, center: &'a RgbaImage, lane: u32) -> Cow<'a, RgbaImage> {
		match lane {
			0 => Cow::Borrowed(corner),
			1 => Cow::Owned(image::imageops::rotate90(corner)),
			2 => Cow::Borrowed(center),
			3 => Cow::Owned(image::imageops::rotate180(corner)),
			4 => Cow::Owned(image::imageops::rotate270(corner)),
			other => panic!("Out of bounds 5k lane {}", other),
		}
	}
}

impl Noteskin for Noteskin5k {
    fn receptor(&self, lane: u32) -> Cow<RgbaImage> {
        Self::get_img(&self.corner_receptor, &self.center_receptor, lane)
	}
	
    fn note(&self, index: usize, lane: u32) -> Cow<RgbaImage> {
        Self::get_img(&self.corner_notes[index], &self.center_notes[index], lane)
    }
}

struct Noteskin6k {
	down_notes: Vec<RgbaImage>,
	down_receptor: RgbaImage,
	down_left_notes: Vec<RgbaImage>,
	down_left_receptor: RgbaImage,
}

impl Noteskin6k {
	pub fn read(
		down_notes_path: &str,
		down_receptor_path: &str,
	) -> Result<Self, Error> {
		let down_to_down_left = |img: &RgbaImage| imageproc::geometric_transformations::rotate_about_center(
			img,
			std::f32::consts::PI * 0.25,
			imageproc::geometric_transformations::Interpolation::Bilinear,
			image::Rgba::from([0, 0, 0, 0]),
		);

		let down_receptor = image::open(down_receptor_path)?.crop(0, 0, 64, 64).into_rgba();
		let down_left_receptor = down_to_down_left(&down_receptor);

		let img = image::open(down_notes_path)?;
		let mut down_notes = Vec::new();
		let mut down_left_notes = Vec::new();
		for y in (0..img.height()).step_by(64) {
			let down_note = img.crop_imm(0, y, 64, 64).into_rgba();
			down_left_notes.push(down_to_down_left(&down_note));
			down_notes.push(down_note);
		}

		Ok(Self { down_left_notes, down_left_receptor, down_notes, down_receptor })
	}

	fn rotate<'a>(down_arrow: &'a RgbaImage, down_left_arrow: &'a RgbaImage, lane: u32) -> Cow<'a, RgbaImage> {
		match lane {
			0 => Cow::Owned(image::imageops::rotate90(down_arrow)),
			1 => Cow::Owned(image::imageops::rotate90(down_left_arrow)),
			2 => Cow::Borrowed(down_arrow),
			3 => Cow::Owned(image::imageops::rotate180(down_arrow)),
			4 => Cow::Owned(image::imageops::rotate180(down_left_arrow)),
			5 => Cow::Owned(image::imageops::rotate270(down_arrow)),
			_ => unimplemented!(),
		}
	}
}

impl Noteskin for Noteskin6k {
	fn receptor(&self, lane: u32) -> Cow<RgbaImage> {
        Self::rotate(&self.down_receptor, &self.down_left_receptor, lane)
	}
	
    fn note(&self, index: usize, lane: u32) -> Cow<RgbaImage> {
        Self::rotate(&self.down_notes[index], &self.down_left_notes[index], lane)
    }
}

struct Pattern {
	/// Each row is a vector of lane numbers. For example a plain jumptrill would be
	/// `vec![vec![0, 1], vec![2, 3], vec![0, 1], vec![2, 3]...]`
	pub rows: Vec<Vec<u32>>,
}

impl Pattern {
	// Determines the keymode (e.g. 4k/5k/6k/...) by adding 1 to the rightmost lane
	pub fn keymode(&self) -> Result<u32, Error> {
		let keymode = 1 + self.rows.iter().flatten().max()
			.ok_or(Error::EmptyPattern)?;
		Ok(keymode)
	}
}

/// Parameter `note_imgs`: a slice of 64x64 images, in the following order: 4ths, 8ths, 12ths,
/// 16ths, 24ths, 32nds, 48ths, 64ths, 192nds
fn render_pattern(
	noteskin: &dyn Noteskin,
	pattern: &Pattern,
	scroll_type: ScrollType,
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
			// Select a note image in the order of 4th-16th-8th-16th (cycle repeats)
			let note_img = noteskin.note([0, 3, 1, 3][i % 4], lane);

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

/// Read noteskin from `noteskin_path`, read the pattern from `pattern_str` and write the generated
/// image into a byte buffer using the PNG format
pub fn generate(
	pattern_str: &str,
	scroll_type: ScrollType,
) -> Result<Vec<u8>, Error> {

	let mut pattern = parse_pattern(pattern_str)?;

	let noteskin: Box<dyn Noteskin> = match pattern.keymode()? {
		0..=4 | 8 => Box::new(NoteskinLdur::read(
			"noteskin/ldur-notes.png", "noteskin/ldur-receptor.png",
			true,
		)?),
		5 => Box::new(Noteskin5k::read(
			"noteskin/5k-center-notes.png", "noteskin/5k-center-receptor.png",
			"noteskin/5k-corner-notes.png", "noteskin/5k-corner-receptor.png"
		)?),
		7 | 9 => Box::new(NoteskinLdur::read(
			"noteskin/bar-notes.png", "noteskin/bar-receptor.png",
			false,
		)?),
		6 => Box::new(Noteskin6k::read(
			"noteskin/ldur-notes.png", "noteskin/ldur-receptor.png",
		)?),
		other @ 10..=u32::MAX => return Err(Error::KeymodeNotImplemented(other)),
	};

	pattern.rows.truncate(100);
	let buffer = render_pattern(noteskin.as_ref(), &pattern, scroll_type)?;
	
	let mut output_buffer = Vec::with_capacity(1_000_000); // allocate 1 MB for the img
	image::DynamicImage::ImageRgba8(buffer).write_to(
		&mut output_buffer,
		image::ImageOutputFormat::Png
	)?;

	Ok(output_buffer)
}