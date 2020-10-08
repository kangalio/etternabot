use thiserror::Error;
use image::GenericImage;

mod pattern;
pub use pattern::*;

mod noteskin;
pub use noteskin::*;

#[derive(Debug, Error)]
pub enum Error {
	#[error("Given pattern is empty")]
	EmptyPattern,
	#[error("Error in the image library")]
	ImageError(#[from] image::ImageError),
	#[error("Can't display a note on lane {lane} using the selected noteskin")]
	NoteskinDoesntSupportLane { lane: usize },
	#[error("{keymode}k not supported by selected noteskin")]
	NoteskinDoesntSupportKeymode { keymode: usize },
	#[error("Lane {human_readable_lane} is invalid in {keymode}k")]
	InvalidLaneForKeymode { human_readable_lane: usize, keymode: usize },
	#[error("Noteskin's texture map doesn't contain all required textures")]
	NoteskinTextureMapTooSmall,
	#[error("{count} sprites would need to be rendered for this pattern, which exceeds the limit of {limit}")]
	TooManySprites { count: usize, limit: usize },
	#[error("Rendered pattern would exceed the limit of {max_width}x{max_height}")]
	ImageTooLarge { width: usize, height: usize, max_width: usize, max_height: usize },
}

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

fn copy_from(this: &mut image::RgbaImage, other: &image::RgbaImage, x: u32, y: u32) -> image::ImageResult<()> {
	// Do bounds checking here so we can use the non-bounds-checking
	// functions to copy pixels.
	if this.width() < other.width() + x || this.height() < other.height() + y {
		return Err(image::ImageError::Parameter(image::error::ParameterError::from_kind(
			image::error::ParameterErrorKind::DimensionMismatch,
		)));
	}

	for i in 0..other.width() {
		for k in 0..other.height() {
			let p = other.get_pixel(i, k);
			this.blend_pixel(i + x, k + y, *p);
		}
	}
	Ok(())
}

fn render_sprite_map(sprite_map: crate::SpriteMap, (max_width, max_height): (usize, usize)) -> Result<image::RgbaImage, crate::Error> {
	let sprite_res = sprite_map.sprite_resolution;

	let max_lane = sprite_map.sprites.iter().map(|s| s.lane).max().ok_or(crate::Error::EmptyPattern)?;
	let max_y_pos = sprite_map.sprites.iter().map(|s| s.y_pos).max().ok_or(crate::Error::EmptyPattern)?;

	// Create an empty image buffer, big enough to fit all the lanes and arrows
	let width = sprite_res * (max_lane + 1);
	let height = ((sprite_res * max_y_pos) as f32 * sprite_map.vertical_spacing_multiplier) as usize + sprite_res;
	if width > max_width || height > max_height {
		return Err(crate::Error::ImageTooLarge { width, height, max_width, max_height });
	}
	let mut buffer = image::ImageBuffer::new(width as u32, height as u32);

	for sprite in sprite_map.sprites {
		let x = sprite.lane * sprite_res;
		let y = ((sprite.y_pos * sprite_res) as f32 * sprite_map.vertical_spacing_multiplier) as usize;
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
	pub pattern: &'a [(SimplePattern, usize)],
	pub max_image_dimensions: (usize, usize),
	pub max_sprites: usize,
}

/// pattern: List of simple patterns and their snap represented as the number of 192nd-steps
pub fn draw_pattern(recipe: PatternRecipe<'_>) -> Result<image::RgbaImage, Error> {
	let PatternRecipe { noteskin, scroll_direction, keymode, vertical_spacing_multiplier, pattern,
		max_image_dimensions, max_sprites } = recipe;

	let mut rows = Vec::new();
	let mut row_number = 0;
	for &(ref pattern, snap) in pattern {
		for row_data in &pattern.rows {
			rows.push((row_data, row_number));
			row_number += snap;
		}
	}
	let highest_row = rows.iter().map(|&(_, row_number)| row_number).max().unwrap_or(0);

	let mut sprites = Vec::new();

	// place receptors first, to not overshadow any notes
	let receptor_y_pos = match scroll_direction {
		etterna::ScrollDirection::Upscroll => 0,
		etterna::ScrollDirection::Downscroll => highest_row,
	};
	for lane in 0..keymode {
		sprites.push(Sprite { lane, y_pos: receptor_y_pos, image: noteskin.receptor(lane, keymode)? });
	}

	for (row_data, row_number) in rows {
		for &note_lane in row_data {
			let note_lane = note_lane.column_number_with_keymode(keymode as u32);

			sprites.push(Sprite {
				lane: note_lane as usize,
				y_pos: match scroll_direction {
					etterna::ScrollDirection::Upscroll => row_number,
					etterna::ScrollDirection::Downscroll => highest_row - row_number,
				},
				image: noteskin.note(note_lane as usize, keymode, etterna::Snap::from_row(row_number))?,
			});
		}
	}

	if sprites.len() > max_sprites {
		return Err(Error::TooManySprites { count: sprites.len(), limit: max_sprites });
	}

	let smallest_snap = pattern.iter().map(|&(_, snap)| snap).min()
		.ok_or(Error::EmptyPattern)?;

	render_sprite_map(SpriteMap {
		sprites,
		sprite_resolution: noteskin.sprite_resolution(),
		vertical_spacing_multiplier: (1.0 / smallest_snap as f32) * vertical_spacing_multiplier,
	}, max_image_dimensions)
}