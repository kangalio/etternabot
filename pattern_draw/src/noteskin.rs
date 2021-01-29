use image::GenericImageView;
use std::convert::TryInto;

fn iterate_center_column_of_texture_map(
	texture_map: &image::RgbaImage,
	sprite_resolution: usize,
) -> impl Iterator<Item = image::RgbaImage> + '_ {
	let num_columns = texture_map.width() as usize / sprite_resolution;
	let center_column = (num_columns - 1) / 2;

	(0..(texture_map.height() as usize / sprite_resolution)).map(move |row| {
		texture_map
			.view(
				(center_column * sprite_resolution) as u32, // x
				(row * sprite_resolution) as u32,           // y
				sprite_resolution as u32,                   // width
				sprite_resolution as u32,                   // height
			)
			.to_image()
	})
}

/// Parameter must be a row of sprites next to each other
fn middle_texture(texture_map: &image::RgbaImage) -> Result<image::RgbaImage, crate::Error> {
	iterate_center_column_of_texture_map(texture_map, texture_map.height() as usize)
		.next()
		.ok_or(crate::Error::NoteskinTextureMapTooSmall)
}

// Rotate a texture of a down-facing arrow to face down-left
fn rotate_clockwise_by(img: &image::RgbaImage, degrees: u32) -> image::RgbaImage {
	imageproc::geometric_transformations::rotate_about_center(
		img,
		std::f32::consts::PI * (degrees as f32 / 180.0),
		imageproc::geometric_transformations::Interpolation::Bilinear,
		image::Rgba::from([0, 0, 0, 0]),
	)
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
	LdurWith6k {
		receptors: [image::RgbaImage; 6],
		notes: [[image::RgbaImage; 6]; 8], // first four are LDUR, then come left-up and right-up
		mine: image::RgbaImage,
	},
	MonoSnapLdur {
		receptors: [image::RgbaImage; 4],
		notes: [image::RgbaImage; 4],
		mine: image::RgbaImage,
	},
	Pump {
		receptors: [image::RgbaImage; 5],
		notes: [[image::RgbaImage; 5]; 8],
		mine: image::RgbaImage,
	},
	Bar {
		receptor: image::RgbaImage,
		notes: [image::RgbaImage; 8],
		mine: image::RgbaImage,
	},
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
		mine_path: &str,
	) -> Result<Self, crate::Error> {
		// we use the middle frame of the animations
		let mine = middle_texture(&image::open(mine_path)?.into_rgba())?;
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
					.map(|(center_note, corner_note)| {
						[
							corner_note.clone(),
							image::imageops::rotate90(&corner_note),
							center_note,
							image::imageops::rotate180(&corner_note),
							image::imageops::rotate270(&corner_note),
						]
					})
					.collect::<Vec<_>>()
					.into_boxed_slice()
					.try_into()
					.map_err(|_| crate::Error::NoteskinTextureMapTooSmall)?;
					*boxed
				},
				mine,
			},
		})
	}

	pub fn read_ldur_with_6k(
		sprite_resolution: usize,
		notes_path: &str,
		receptor_path: &str,
		mine_path: &str,
	) -> Result<Self, crate::Error> {
		// we use the middle frame of the animations
		let mine = middle_texture(&image::open(mine_path)?.into_rgba())?;
		let receptor = middle_texture(&image::open(receptor_path)?.into_rgba())?;
		let notes = image::open(notes_path)?.into_rgba();

		Ok(Self {
			sprite_resolution,
			textures: Textures::LdurWith6k {
				receptors: [
					image::imageops::rotate90(&receptor),
					receptor.clone(),
					image::imageops::rotate180(&receptor),
					image::imageops::rotate270(&receptor),
					rotate_clockwise_by(&receptor, 135), // rotate down -> up-left
					rotate_clockwise_by(&receptor, 225), // rotate down -> up-right
				],
				notes: {
					let boxed: Box<_> =
						iterate_center_column_of_texture_map(&notes, sprite_resolution)
							.map(|note| {
								[
									image::imageops::rotate90(&note),
									note.clone(),
									image::imageops::rotate180(&note),
									image::imageops::rotate270(&note),
									rotate_clockwise_by(&note, 135), // rotate down -> up-left
									rotate_clockwise_by(&note, 225), // rotate down -> up-right
								]
							})
							.collect::<Vec<_>>()
							.into_boxed_slice()
							.try_into()
							.map_err(|_| crate::Error::NoteskinTextureMapTooSmall)?;
					*boxed
				},
				mine,
			},
		})
	}

	#[allow(clippy::too_many_arguments)] // ehhhhhhh this is fine
	pub fn read_ldur(
		sprite_resolution: usize,
		left_note_path: &str,
		left_receptor_path: &str,
		down_note_path: &str,
		down_receptor_path: &str,
		up_note_path: &str,
		up_receptor_path: &str,
		right_note_path: &str,
		right_receptor_path: &str,
		mine_path: &str,
	) -> Result<Self, crate::Error> {
		Ok(Self {
			sprite_resolution,
			textures: Textures::MonoSnapLdur {
				notes: [
					image::open(left_note_path)?.into_rgba(),
					image::open(down_note_path)?.into_rgba(),
					image::open(up_note_path)?.into_rgba(),
					image::open(right_note_path)?.into_rgba(),
				],
				receptors: [
					image::open(left_receptor_path)?.into_rgba(),
					image::open(down_receptor_path)?.into_rgba(),
					image::open(up_receptor_path)?.into_rgba(),
					image::open(right_receptor_path)?.into_rgba(),
				],
				mine: image::open(mine_path)?.into_rgba(),
			},
		})
	}

	pub fn read_bar(
		sprite_resolution: usize,
		notes_path: &str,
		receptor_path: &str,
		mine_path: &str,
	) -> Result<Self, crate::Error> {
		// we use the middle frame of the animations
		let mine = middle_texture(&image::open(mine_path)?.into_rgba())?;
		let receptor = middle_texture(&image::open(receptor_path)?.into_rgba())?;
		let notes = image::open(notes_path)?.into_rgba();

		Ok(Self {
			sprite_resolution,
			textures: Textures::Bar {
				receptor,
				notes: {
					let boxed: Box<_> =
						iterate_center_column_of_texture_map(&notes, sprite_resolution)
							.collect::<Vec<_>>()
							.into_boxed_slice()
							.try_into()
							.map_err(|_| crate::Error::NoteskinTextureMapTooSmall)?;
					*boxed
				},
				mine,
			},
		})
	}

	fn check_keymode(&self, lane: usize, keymode: usize) -> Result<(), crate::Error> {
		if lane >= keymode {
			return Err(crate::Error::InvalidLaneForKeymode {
				human_readable_lane: lane + 1,
				keymode,
			});
		}

		let keymode_is_supported = match self.textures {
			Textures::LdurWith6k { .. } => matches!(keymode, 3 | 4 | 6 | 8),
			Textures::MonoSnapLdur { .. } => matches!(keymode, 3 | 4 | 8),
			Textures::Pump { .. } => matches!(keymode, 5 | 10),
			// honestly there's no reason not to make the bar skin accept all keymodes. And if it
			// didn't it would be impossible to draw any patterns with more than 10 lanes
			// Textures::Bar { .. } => matches!(keymode, 7 | 9),
			Textures::Bar { .. } => true,
		};
		if keymode_is_supported {
			Ok(())
		} else {
			Err(crate::Error::NoteskinDoesntSupportKeymode { keymode })
		}
	}

	fn lane_to_note_array_index(&self, lane: usize, keymode: usize) -> Result<usize, crate::Error> {
		self.check_keymode(lane, keymode)?;

		Ok(match self.textures {
			Textures::LdurWith6k { .. } => match keymode {
				6 => [0, 4, 1, 2, 5, 3][lane],
				3 => [0, 1, 3][lane],
				_ => lane % 4,
			},
			Textures::MonoSnapLdur { .. } => match keymode {
				3 => [0, 1, 3][lane],
				_ => lane % 4,
			},
			Textures::Pump { .. } => lane % 5,
			Textures::Bar { .. } => 0, // not applicable, but let's return something anyway
		})
	}

	/// The returned image has the resolution NxN, where N can be obtained with `sprite_resolution()`
	pub fn note(
		&self,
		lane: usize,
		keymode: usize,
		snap: etterna::Snap,
	) -> Result<&image::RgbaImage, crate::Error> {
		self.check_keymode(lane, keymode)?;

		Ok(match &self.textures {
			Textures::LdurWith6k { notes, .. } => {
				&notes[snap_to_texture_index(snap)][self.lane_to_note_array_index(lane, keymode)?]
			}
			Textures::MonoSnapLdur { notes, .. } => {
				&notes[self.lane_to_note_array_index(lane, keymode)?]
			}
			Textures::Pump { notes, .. } => {
				&notes[snap_to_texture_index(snap)][self.lane_to_note_array_index(lane, keymode)?]
			}
			Textures::Bar { notes, .. } => &notes[snap_to_texture_index(snap)],
		})
	}

	/// The returned image has the resolution NxN, where N can be obtained with `sprite_resolution()`
	pub fn receptor(&self, lane: usize, keymode: usize) -> Result<&image::RgbaImage, crate::Error> {
		self.check_keymode(lane, keymode)?;

		Ok(match &self.textures {
			Textures::LdurWith6k { receptors, .. } => {
				&receptors[self.lane_to_note_array_index(lane, keymode)?]
			}
			Textures::MonoSnapLdur { receptors, .. } => {
				&receptors[self.lane_to_note_array_index(lane, keymode)?]
			}
			Textures::Pump { receptors, .. } => {
				&receptors[self.lane_to_note_array_index(lane, keymode)?]
			}
			Textures::Bar { receptor, .. } => &receptor,
		})
	}

	/// The returned image has the resolution NxN, where N can be obtained with `sprite_resolution()`
	pub fn mine(&self) -> Result<&image::RgbaImage, crate::Error> {
		Ok(match &self.textures {
			Textures::LdurWith6k { mine, .. } => &mine,
			Textures::MonoSnapLdur { mine, .. } => &mine,
			Textures::Pump { mine, .. } => &mine,
			Textures::Bar { mine, .. } => &mine,
		})
	}

	pub fn sprite_resolution(&self) -> usize {
		self.sprite_resolution
	}

	fn for_each_texture(&mut self, mut f: impl FnMut(&mut image::RgbaImage)) {
		match &mut self.textures {
			Textures::LdurWith6k {
				mine,
				notes,
				receptors,
			} => {
				f(mine);
				for row in notes {
					for note in row {
						f(note);
					}
				}
				for receptor in receptors {
					f(receptor);
				}
			}
			Textures::MonoSnapLdur {
				mine,
				notes,
				receptors,
			} => {
				f(mine);
				for note in notes {
					f(note);
				}
				for receptor in receptors {
					f(receptor);
				}
			}
			Textures::Pump {
				mine,
				notes,
				receptors,
			} => {
				f(mine);
				for row in notes {
					for note in row {
						f(note);
					}
				}
				for receptor in receptors {
					f(receptor);
				}
			}
			Textures::Bar {
				mine,
				notes,
				receptor,
			} => {
				f(mine);
				for note in notes {
					f(note);
				}
				f(receptor);
			}
		}
	}

	pub fn resize_sprites(&mut self, sprite_resolution: u32) {
		self.for_each_texture(|texture| {
			// Triangle (aka bilinear) is the fastest resize algorithm that doesn't look garbage
			*texture = image::imageops::resize(
				texture,
				sprite_resolution,
				sprite_resolution,
				image::imageops::FilterType::Triangle,
			);
		});
		self.sprite_resolution = sprite_resolution as usize;
	}

	pub fn turn_sprites_upside_down(&mut self) {
		self.for_each_texture(|texture| image::imageops::rotate180_in_place(texture));
	}
}
