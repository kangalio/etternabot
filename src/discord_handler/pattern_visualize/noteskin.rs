use super::*;

pub trait Noteskin {
	fn receptor(&self, lane: u32) -> Cow<RgbaImage>;
	fn note_unchecked(&self, index: usize, lane: u32) -> Cow<RgbaImage>; // this method may panic if index is out of bounds

	fn note(&self, index: usize, lane: u32) -> Cow<RgbaImage> {
		self.note_unchecked(index.min(7), lane)
	}
}

pub struct NoteskinLdur {
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

	fn note_unchecked(&self, index: usize, lane: u32) -> Cow<RgbaImage> {
		Self::rotate(&self.notes[index], lane, self.rotate)
	}
}

pub struct Noteskin5k {
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
	
    fn note_unchecked(&self, index: usize, lane: u32) -> Cow<RgbaImage> {
        Self::get_img(&self.corner_notes[index], &self.center_notes[index], lane)
    }
}

pub struct Noteskin6k {
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
	
    fn note_unchecked(&self, index: usize, lane: u32) -> Cow<RgbaImage> {
        Self::rotate(&self.down_notes[index], &self.down_left_notes[index], lane)
    }
}