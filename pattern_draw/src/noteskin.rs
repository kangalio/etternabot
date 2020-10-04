use std::convert::TryInto;
use image::GenericImageView;

fn iterate_center_column_of_texture_map(
	texture_map: &image::RgbaImage,
	sprite_resolution: usize,
) -> impl Iterator<Item = image::RgbaImage> + '_ {
	let num_columns = texture_map.width() as usize / sprite_resolution;
	let center_column = (num_columns - 1) / 2;

	(0..(texture_map.height() as usize / sprite_resolution))
		.map(move |row| texture_map.view(
			(center_column * sprite_resolution) as u32, // x
			(row * sprite_resolution) as u32, // y
			sprite_resolution as u32, // width
			sprite_resolution as u32, // height
		).to_image())
}

/// Parameter must be a row of sprites next to each other
fn middle_texture(texture_map: &image::RgbaImage) -> Result<image::RgbaImage, crate::Error> {
	iterate_center_column_of_texture_map(texture_map, texture_map.height() as usize).next()
		.ok_or(crate::Error::NoteskinTextureMapTooSmall)
}

// The returned index ranges from 0 to 7, so it can be used to index into a [T; 8]
fn snap_to_texture_index(snap: etterna::Snap) -> usize {
	match snap {
		etterna::Snap::_4th => 0,
		etterna::Snap::_8th => 1,
		etterna::Snap::_12th => 2,
		etterna::Snap::_16th => 3,
		etterna::Snap::_24th => 4,
		etterna::Snap::_32th => 5,
		etterna::Snap::_48th => 6,
		etterna::Snap::_64th => 7,
		etterna::Snap::_192th => 7,
	}
}

enum Textures {
	Ldur {
		receptors: [image::RgbaImage; 4],
		notes: [[image::RgbaImage; 4]; 8],
	},
	Pump {
		receptors: [image::RgbaImage; 5],
		notes: [[image::RgbaImage; 5]; 8],
	},
	Bar {
		receptor: image::RgbaImage,
		notes: [image::RgbaImage; 8],
	}
}

pub struct Noteskin {
	sprite_resolution: usize,
	textures: Textures,
}

impl Noteskin {
	pub fn read_pump(
		sprite_resolution: usize,
		center_notes_path: &str,
		center_receptor_path: &str,
		corner_notes_path: &str,
		corner_receptor_path: &str,
	) -> Result<Self, crate::Error> {
		// we use the middle frame of the animations
		let center_receptor = middle_texture(&image::open(center_receptor_path)?.into_rgba())?;
		let corner_receptor = middle_texture(&image::open(corner_receptor_path)?.into_rgba())?;
		let center_notes = image::open(center_notes_path)?.into_rgba();
		let corner_notes = image::open(corner_notes_path)?.into_rgba();

		Ok(Self {
			sprite_resolution,
			textures: Textures::Pump {
				receptors: [
					corner_receptor.clone(),
					image::imageops::rotate90(&corner_receptor),
					center_receptor,
					image::imageops::rotate180(&corner_receptor),
					image::imageops::rotate270(&corner_receptor),
				],
				notes: {
					let boxed: Box<_> = Iterator::zip(
						iterate_center_column_of_texture_map(&center_notes, sprite_resolution),
						iterate_center_column_of_texture_map(&corner_notes, sprite_resolution),
					)
						.map(|(center_note, corner_note)| [
							corner_note.clone(),
							image::imageops::rotate90(&corner_note),
							center_note,
							image::imageops::rotate180(&corner_note),
							image::imageops::rotate270(&corner_note),
						])
						.collect::<Vec<_>>().into_boxed_slice().try_into()
						.map_err(|_| crate::Error::NoteskinTextureMapTooSmall)?;
					*boxed
				},
			}
		})
	}

	pub fn read_ldur(
		sprite_resolution: usize,
		notes_path: &str,
		receptor_path: &str,
	) -> Result<Self, crate::Error> {
		// we use the middle frame of the animations
		let receptor = middle_texture(&image::open(receptor_path)?.into_rgba())?;
		let notes = image::open(notes_path)?.into_rgba();

		Ok(Self {
			sprite_resolution,
			textures: Textures::Ldur {
				receptors: [
					image::imageops::rotate90(&receptor),
					receptor.clone(),
					image::imageops::rotate180(&receptor),
					image::imageops::rotate270(&receptor),
				],
				notes: {
					let boxed: Box<_> = iterate_center_column_of_texture_map(&notes, sprite_resolution)
						.map(|note| [
							image::imageops::rotate90(&note),
							note.clone(),
							image::imageops::rotate180(&note),
							image::imageops::rotate270(&note),
						])
						.collect::<Vec<_>>().into_boxed_slice().try_into()
						.map_err(|_| crate::Error::NoteskinTextureMapTooSmall)?;
					*boxed
				},
			}
		})
	}

	pub fn read_bar(
		sprite_resolution: usize,
		notes_path: &str,
		receptor_path: &str,
	) -> Result<Self, crate::Error> {
		// we use the middle frame of the animations
		let receptor = middle_texture(&image::open(receptor_path)?.into_rgba())?;
		let notes = image::open(notes_path)?.into_rgba();

		Ok(Self {
			sprite_resolution,
			textures: Textures::Bar {
				receptor,
				notes: {
					let boxed: Box<_> = iterate_center_column_of_texture_map(&notes, sprite_resolution)
						.collect::<Vec<_>>().into_boxed_slice().try_into()
						.map_err(|_| crate::Error::NoteskinTextureMapTooSmall)?;
					*boxed
				},
			}
		})
	}

	/// The returned image has the resolution NxN, where N can be obtained with `sprite_resolution()`
	pub fn note(&self, lane: usize, snap: etterna::Snap) -> Result<&image::RgbaImage, crate::Error> {
		Ok(match &self.textures {
			Textures::Ldur { notes, .. } => &notes[snap_to_texture_index(snap)][lane % 4],
			Textures::Pump { notes, .. } => &notes[snap_to_texture_index(snap)][lane % 5],
			Textures::Bar { notes, .. } => &notes[snap_to_texture_index(snap)],
		})
	}

	/// The returned image has the resolution NxN, where N can be obtained with `sprite_resolution()`
	pub fn receptor(&self, lane: usize) -> Result<&image::RgbaImage, crate::Error> {
		Ok(match &self.textures {
			Textures::Ldur { receptors, .. } => &receptors[lane % 4],
			Textures::Pump { receptors, .. } => &receptors[lane & 5],
			Textures::Bar { receptor, .. } => &receptor,
		})
	}

	pub fn sprite_resolution(&self) -> usize {
		self.sprite_resolution
	}
}