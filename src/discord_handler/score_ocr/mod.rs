#![allow(clippy::match_ref_pats)]

use leptess::LepTess;
use etternaonline_api::{Difficulty, Rate, TapJudgements};
use thiserror::Error;

pub const MINIMUM_EQUALITY_SCORE_TO_BE_PROBABLY_EQUAL: i32 = 10;

#[derive(Debug, Error)]
pub enum Error {
	#[error("Failed to initialize Tesseract: {0:?}")]
	TesseractInit(#[from] leptess::tesseract::TessInitError),
	#[error("Leptonica failed reading the provided image")]
	CouldNotReadImage,
}

// not needed rn
// #[derive(Copy, Clone, Debug, Eq, PartialEq, Default, Hash)]
// pub struct Similarity {
// 	pub tested: u32,
// 	pub matches: u32,
// }

// impl Similarity {
// 	pub fn matched_proportion(self) -> f32 {
// 		if self.tested == 0 {
// 			return 0.0;
// 		}

// 		self.matches as f32 / self.tested as f32
// 	}
// }

fn recognize_rect<T>(
	lt: &mut LepTess,
	rect_x: u32, rect_y: u32, rect_w: u32, rect_h: u32, // the coordinates are in 1920x1080 format
	processor: impl FnOnce(&str) -> Option<T>
) -> Option<T> {
	let (img_w, img_h) = lt.get_image_dimensions().expect("hey caller, you should've set an image by now");

	lt.set_rectangle(&leptess::leptonica::Box::new(
		(rect_x * img_w / 1920) as i32,
		(rect_y * img_h / 1080) as i32,
		(rect_w * img_w / 1920) as i32,
		(rect_h * img_h / 1080) as i32,
	).unwrap());
	let text = lt.get_utf8_text().ok()?;
	let text = text.trim();
	println!("Recognized string: {}", text);
	processor(text)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EvaluationScreenData {
	pub rate: Option<Rate>,
	pub pack: Option<String>,
	pub eo_username: Option<String>,
	pub song: Option<String>,
	pub artist: Option<String>,
	/// From 0.0 to 100.0
	pub wifescore: Option<f32>,
	pub msd: Option<f32>,
	pub ssr: Option<f32>,
	pub judgements: Option<TapJudgements>,
	pub difficulty: Option<Difficulty>,
}

impl EvaluationScreenData {
	// not needed rn
	// pub fn recognize_from_image_path(path: &str) -> Result<Self, Error> {
	// 	Self::recognize(|lt| lt.set_image(path))
	// }

	pub fn recognize_from_image_bytes(bytes: &[u8]) -> Result<Self, Error> {
		Self::recognize(|lt| lt.set_image_from_mem(bytes))
	}

	pub fn recognize(
		mut image_setter: impl FnMut(&mut LepTess) -> Option<()>
	) -> Result<Self, Error> {
		let mut eng_lt = LepTess::new(Some("ocr_data"), "eng")?;
		let mut num_lt = LepTess::new(Some("ocr_data"), "digitsall_layer")?;

		// that's apparently the full screen dpi and our images are fullscreen so let's use this value
		let dpi = 96;

		(image_setter)(&mut eng_lt).ok_or(Error::CouldNotReadImage)?;
		eng_lt.set_fallback_source_resolution(dpi);
		(image_setter)(&mut num_lt).ok_or(Error::CouldNotReadImage)?;
		num_lt.set_fallback_source_resolution(dpi);

		Ok(Self {
			rate: recognize_rect(&mut num_lt, 914, 371, 98, 19, |s| {
				Rate::from_f32(s.parse().ok()?)
			}),
			pack: recognize_rect(&mut eng_lt, 241, 18, 1677, 55, |s| {
				Some(s.to_owned())
			}),
			eo_username: recognize_rect(&mut eng_lt, 461, 1004, 1111, 40, |s| {
				let (eo_username, eo_rating, eo_rank): (String, String, String);
				text_io::try_scan!(@impl or_none; s.bytes() => "Logged in as {} ({}: #{})",
					eo_username, eo_rating, eo_rank);
				Some(eo_username)
			}),
			song: recognize_rect(&mut eng_lt, 760, 322, 406, 32, |s| {
				Some(s.to_owned())
			}),
			artist: recognize_rect(&mut eng_lt, 747, 350, 417, 25, |s| {
				Some(s.to_owned())
			}),
			wifescore: recognize_rect(&mut num_lt, 53, 339, 128, 40, |s| {
				Some(s.trim().parse().ok()?)
			}),
			msd: recognize_rect(&mut num_lt, 33, 385, 209, 51, |s| {
				Some(s.trim().parse().ok()?)
			}),
			ssr: recognize_rect(&mut num_lt, 535, 385, 209, 51, |s| {
				Some(s.trim().parse().ok()?)
			}),
			judgements: recognize_rect(&mut num_lt, 1422, 171, 308, 21, |s| {
				let judgements: Vec<u32> = s
					.split('/')
					.filter_map(|s| s.trim().parse().ok())
					.collect();
				
				match judgements.as_slice() {
					&[marvelouses, perfects, greats, goods, bads, misses] => Some(TapJudgements { marvelouses, perfects, greats, goods, bads, misses }),
					&[marvelouses, perfects, greats, goods, bads] => Some(TapJudgements { marvelouses, perfects, greats, goods, bads, misses: 0 }),
					&[marvelouses, perfects, greats, goods] => Some(TapJudgements { marvelouses, perfects, greats, goods, bads: 0, misses: 0 }),
					&[marvelouses, perfects, greats] => Some(TapJudgements { marvelouses, perfects, greats, goods: 0, bads: 0, misses: 0 }),
					_ => None,
				}
			}),
			difficulty: recognize_rect(&mut eng_lt, 646, 324, 100, 56, |s| {
				Difficulty::from_short_string(s)
			}),
		})
	}

	pub fn equality_score(&self, other: &Self) -> i32 {
		let mut score: i32 = 0;

		macro_rules! compare {
			($a:expr, $b:expr, $weight:expr, $equality_check:expr) => {
				if let (Some(a), Some(b)) = (&$a, &$b) {
					println!("{:?} == {:?} ?", a, b);
					if $equality_check(a, b) {
						println!("{} matches! Adding {} points", stringify!($a), $weight);
						score += $weight;
					} else {
						// score -= $weight;
					}
				}
			};
			($a:expr, $b:expr, $weight:expr) => {
				compare!($a, $b, $weight, |a, b| a == b);
			};
			($a:expr, $b:expr, $weight:expr, ~$epsilon:expr) => {
				compare!($a, $b, $weight, |a: &f32, b: &f32| (a - b).abs() <= $epsilon);
			};
		}
		compare!(self.rate, other.rate, 2);
		compare!(self.pack, other.pack, 3);
		compare!(self.eo_username, other.eo_username, 5);
		compare!(self.song, other.song, 6);
		compare!(self.artist, other.artist, 3);
		compare!(self.wifescore, other.wifescore, 5, ~0.01);
		compare!(self.msd, other.msd, 6, ~0.01);
		compare!(self.ssr, other.ssr, 6, ~0.01);
		compare!(self.difficulty, other.difficulty, 2);
		compare!(
			self.judgements.as_ref().map(|j| j.marvelouses),
			other.judgements.as_ref().map(|j| j.marvelouses),
			5
		);
		compare!(
			self.judgements.as_ref().map(|j| j.perfects),
			other.judgements.as_ref().map(|j| j.perfects),
			5
		);
		compare!(
			self.judgements.as_ref().map(|j| j.greats),
			other.judgements.as_ref().map(|j| j.greats),
			5
		);
		compare!(
			self.judgements.as_ref().map(|j| j.goods),
			other.judgements.as_ref().map(|j| j.goods),
			2
		);
		compare!(
			self.judgements.as_ref().map(|j| j.bads),
			other.judgements.as_ref().map(|j| j.bads),
			2
		);
		compare!(
			self.judgements.as_ref().map(|j| j.misses),
			other.judgements.as_ref().map(|j| j.misses),
			3
		);

		println!("Got total {} points", score);
		println!();

		score
	}
}