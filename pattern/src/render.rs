use image::Pixel;

use super::*;

struct Sprite<'a> {
	lane: usize,
	y_pos: usize,
	image: &'a image::RgbaImage,
}

struct SpriteMap<'a> {
	sprites: Vec<Sprite<'a>>,
	sprite_resolution: usize,
	vertical_spacing_multiplier: f32,
}

fn copy_from(
	this: &mut image::RgbaImage,
	other: &image::RgbaImage,
	x: u32,
	y: u32,
) -> image::ImageResult<()> {
	// Do bounds checking here so we can use the non-bounds-checking
	// functions to copy pixels.
	if this.width() < other.width() + x || this.height() < other.height() + y {
		return Err(image::ImageError::Parameter(
			image::error::ParameterError::from_kind(
				image::error::ParameterErrorKind::DimensionMismatch,
			),
		));
	}

	for i in 0..other.width() {
		for k in 0..other.height() {
			let p = other.get_pixel(i, k);
			this.get_pixel_mut(i + x, k + y).blend(p);
		}
	}
	Ok(())
}

fn render_sprite_map(
	sprite_map: SpriteMap<'_>,
	(max_width, max_height): (usize, usize),
) -> Result<image::RgbaImage, Error> {
	let sprite_res = sprite_map.sprite_resolution;

	let max_lane = sprite_map
		.sprites
		.iter()
		.map(|s| s.lane)
		.max()
		.ok_or(Error::EmptyPattern)?;
	let max_y_pos = sprite_map
		.sprites
		.iter()
		.map(|s| s.y_pos)
		.max()
		.ok_or(Error::EmptyPattern)?;

	// Create an empty image buffer, big enough to fit all the lanes and arrows
	let width = sprite_res * (max_lane + 1);
	let height = ((sprite_res * max_y_pos) as f32 * sprite_map.vertical_spacing_multiplier)
		as usize + sprite_res;
	if width > max_width || height > max_height {
		return Err(Error::ImageTooLarge {
			width,
			height,
			max_width,
			max_height,
		});
	}
	let mut buffer = image::ImageBuffer::new(width as u32, height as u32);

	for sprite in sprite_map.sprites {
		let x = sprite.lane * sprite_res;
		let y =
			((sprite.y_pos * sprite_res) as f32 * sprite_map.vertical_spacing_multiplier) as usize;
		// buffer.copy_from(sprite.image, x as u32, y as u32)
		// 	.expect("Note image is too large (shouldn't happen)");
		copy_from(&mut buffer, sprite.image, x as u32, y as u32)
			.expect("Note image is too large (shouldn't happen)");
	}

	Ok(buffer)
}

pub struct PatternRecipe<'a> {
	pub noteskin: &'a Noteskin,
	pub scroll_direction: etterna::ScrollDirection,
	pub keymode: usize,
	pub vertical_spacing_multiplier: f32,
	// List of pattern segments and their snap
	pub pattern: &'a [(Pattern, FractionalSnap)],
	pub max_image_dimensions: (usize, usize),
	pub max_sprites: usize,
}

/// pattern: List of simple patterns and their snap represented as the number of 192nd-steps
pub fn draw_pattern(recipe: PatternRecipe<'_>) -> Result<image::RgbaImage, Error> {
	let PatternRecipe {
		noteskin,
		scroll_direction,
		keymode,
		vertical_spacing_multiplier,
		pattern,
		max_image_dimensions,
		max_sprites,
	} = recipe;

	let mut rows = Vec::new();
	let mut row_number = 0;
	for (pattern, snap) in pattern {
		let mut snap_192nd_intervals = snap.iter_192nd_intervals();

		for row_data in &pattern.rows {
			rows.push((row_data, row_number));
			row_number += snap_192nd_intervals.next_interval() as usize;
		}
	}
	let highest_row = rows
		.iter()
		.map(|&(_, row_number)| row_number)
		.max()
		.unwrap_or(0);

	let mut sprites = Vec::new();

	// place receptors first, to not overshadow any notes
	let receptor_y_pos = match scroll_direction {
		etterna::ScrollDirection::Upscroll => 0,
		etterna::ScrollDirection::Downscroll => highest_row,
	};
	for lane in 0..keymode {
		sprites.push(Sprite {
			lane,
			y_pos: receptor_y_pos,
			image: noteskin.receptor(lane, keymode)?,
		});
	}

	for (row_data, row_number) in rows {
		for &(note_lane, note_type) in &row_data.notes {
			let note_lane = note_lane.column_number_with_keymode(keymode as u32);

			sprites.push(Sprite {
				lane: note_lane as usize,
				y_pos: match scroll_direction {
					etterna::ScrollDirection::Upscroll => row_number,
					etterna::ScrollDirection::Downscroll => highest_row - row_number,
				},
				image: match note_type {
					NoteType::Tap => noteskin.note(
						note_lane as usize,
						keymode,
						etterna::Snap::from_row(row_number as _),
					)?,
					NoteType::Mine => noteskin.mine()?,
					NoteType::Hold { .. } => return Err(Error::HoldsAreUnsupported),
				},
			});
		}
	}

	if sprites.len() > max_sprites {
		return Err(Error::TooManySprites {
			count: sprites.len(),
			limit: max_sprites,
		});
	}

	let highest_snap = pattern
		.iter()
		.map(|&(_, snap)| snap.snap_number())
		.min()
		.ok_or(Error::EmptyPattern)?;
	let smallest_192nd_interval = 192.0 / highest_snap as f32;

	render_sprite_map(
		SpriteMap {
			sprites,
			sprite_resolution: noteskin.sprite_resolution(),
			vertical_spacing_multiplier: (1.0 / smallest_192nd_interval)
				* vertical_spacing_multiplier,
		},
		max_image_dimensions,
	)
}
