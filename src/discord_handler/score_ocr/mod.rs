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

fn recognize_rect<T>(
	lt: &mut LepTess,
	rect_x: u32, rect_y: u32, rect_w: u32, rect_h: u32, // the coordinates are in 1920x1080 format
	processor: impl FnOnce(&str) -> Option<T>
) -> Option<T> {
	let (actual_img_w, actual_img_h) = lt.get_image_dimensions()
		.expect("hey caller, you should've set an image by now");

	// Make everything float for easier math
	let (actual_img_w, actual_img_h) = (actual_img_w as f32, actual_img_h as f32);
	let (mut rect_x, mut rect_y) = (rect_x as f32, rect_y as f32);
	let (mut rect_w, mut rect_h) = (rect_w as f32, rect_h as f32);

	// Normalize to height=1080
	let height_multiplier = 1080.0 / actual_img_h;
	rect_x *= height_multiplier;
	rect_y *= height_multiplier;
	rect_w *= height_multiplier;
	rect_h *= height_multiplier;
	let img_w = actual_img_w * height_multiplier;

	if rect_x + rect_w / 2.0 > 1920.0 / 2.0 { // if on right half, assume anchored to right side
		rect_x += img_w - 1920.0;
	}

	let bounding_box = &leptess::leptonica::Box::new(
		(rect_x / height_multiplier) as i32,
		(rect_y / height_multiplier) as i32,
		(rect_w / height_multiplier) as i32,
		(rect_h / height_multiplier) as i32,
	).unwrap();

	println!("{:?}", bounding_box.get_val());

	lt.set_rectangle(bounding_box);
	let text = lt.get_utf8_text().ok()?;
	let text = text.trim();
	println!("Recognized string: {}", text);
	processor(text)
}

fn parse_slash_separated_judgement_string(s: &str) -> Option<TapJudgements> {
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
}

fn recognize_til_death(
	mut eng_lt: &mut LepTess,
	mut num_lt: &mut LepTess,
) -> Result<EvaluationScreenData, Error> {
	Ok(EvaluationScreenData {
		rate: recognize_rect(&mut num_lt, 914, 371, 98, 19, |s| {
			Rate::from_f32(s.parse().ok()?)
		}),
		pack: recognize_rect(&mut eng_lt, 241, 18, 1677, 55, |s| {
			Some(s.to_owned())
		}),
		eo_username: recognize_rect(&mut eng_lt, 461, 1004, 1111, 40, |s| {
			let (eo_username, _rest): (String, String);
			text_io::try_scan!(@impl or_none; s.bytes() => "Logged in as {} ({}", eo_username, _rest);
			
			// let (eo_rating, eo_rank): (String, String);
			// text_io::try_scan!(@impl or_none; rest.bytes() => "{}: #{})", eo_rating, eo_rank);

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
		// NOTE - we're reading the judgements from the top-most box from the score boxes in the
		// top right of the eval screen. The problem with this is that those boxes are ordered
		// by wifescore. If the score that was just made was not a PB, it's not at the top, and
		// we're reading _some other score's judgements data here_. HOWEVER!! Due to the fact
		// that EO doesn't save non-PBs, we wouldn't find the score _anyways_ if it's not a PB.
		// So it's not actually a problem that we're not properly recognizing non-PBs.
		judgements: recognize_rect(&mut num_lt, 1422, 171, 308, 21, parse_slash_separated_judgement_string),
		difficulty: recognize_rect(&mut eng_lt, 646, 324, 100, 56, |s| {
			Difficulty::from_short_string(s)
		}),
		date: None, // Til Death doesn't show score date, only current date
	})
}

fn recognize_scwh(
	mut eng_lt: &mut LepTess,
	mut num_lt: &mut LepTess,
) -> Result<EvaluationScreenData, Error> {
	/*
	rate - 
	pack - 
	eo_username - 1567, 786, 287, 55
	song - 
	artist - 
	wifescore - 460, 199, 181, 57
	msd - 85, 156, 85, 32
	ssr - 83, 195, 143, 61
	judgements - 1310, 407, 260, 22
	difficulty - 233, 225, 62, 31
	date - 1399, 920, 454, 49

	title - 78, 96, 952, 51
	subtitle - 78, 146, 952, 43
	*/

	let song_and_rate = recognize_rect(&mut eng_lt, 78, 96, 952, 51, |s| {
		let (song, rate): (String, String);
		text_io::try_scan!(@impl or_none; s.bytes() => "{} ({}x))", song, rate);
		Some((song, rate))
	});
	let (song, rate) = match song_and_rate {
		Some((song, rate)) => (Some(song), etterna::Rate::from_string(&rate)),
		None => (None, None),
	};

	Ok(EvaluationScreenData {
		rate,
		pack: recognize_rect(&mut eng_lt, 1268, 0, 630, 45, |s| {
			Some(s.to_owned())
		}),
		eo_username: recognize_rect(&mut eng_lt, 1567, 786, 287, 55, |s| {
			Some(s.to_owned())
		}),
		song,
		artist: recognize_rect(&mut eng_lt, 163, 146, 871, 43, |s| {
			let artist: String;
			text_io::try_scan!(@impl or_none; s.bytes() => "By: {}", artist);
			Some(artist)
		}),
		wifescore: recognize_rect(&mut num_lt, 460, 199, 181, 57, |s| {
			Some(s.trim().parse().ok()?)
		}),
		msd: recognize_rect(&mut num_lt, 85, 156, 85, 32, |s| {
			Some(s.trim().parse().ok()?)
		}),
		ssr: recognize_rect(&mut num_lt, 83, 195, 143, 61, |s| {
			Some(s.trim().parse().ok()?)
		}),
		// same gotcha applies as in til death
		judgements: recognize_rect(&mut num_lt, 1310, 407, 260, 22, parse_slash_separated_judgement_string),
		difficulty: recognize_rect(&mut eng_lt, 233, 225, 62, 31, |s| {
			Difficulty::from_short_string(s)
		}),
		date: recognize_rect(&mut eng_lt, 1399, 920, 454, 49, |s| {
			Some(s.to_owned())
		}),
	})
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
	pub date: Option<String>,
}

impl EvaluationScreenData {
	// not needed rn
	// pub fn recognize_from_image_path(path: &str) -> Result<Self, Error> {
	// 	Self::recognize(|lt| lt.set_image(path))
	// }

	pub fn recognize_from_image_bytes(bytes: &[u8]) -> Result<Vec<Self>, Error> {
		Self::recognize(|lt| lt.set_image_from_mem(bytes))
	}

	pub fn recognize(
		mut image_setter: impl FnMut(&mut LepTess) -> Option<()>
	) -> Result<Vec<Self>, Error> {
		let mut eng_lt = LepTess::new(Some("ocr_data"), "eng")?;
		let mut num_lt = LepTess::new(Some("ocr_data"), "digitsall_layer")?;

		// that's apparently the full screen dpi and our images are fullscreen so let's use this value
		let dpi = 96;

		(image_setter)(&mut eng_lt).ok_or(Error::CouldNotReadImage)?;
		eng_lt.set_fallback_source_resolution(dpi);
		(image_setter)(&mut num_lt).ok_or(Error::CouldNotReadImage)?;
		num_lt.set_fallback_source_resolution(dpi);

		Ok(vec![
			recognize_til_death(&mut eng_lt, &mut num_lt)?,
			recognize_scwh(&mut eng_lt, &mut num_lt)?,
		])
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
		compare!(self.date, other.date, 2);
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