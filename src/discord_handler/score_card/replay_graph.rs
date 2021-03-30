#![allow(clippy::type_complexity)] // type complexity is not my fault

use etterna::Wife;
use etternaonline_api::v2 as eo;
use plotters::{
	prelude::*,
	style::text_anchor::{HPos, Pos, VPos}, /*style::RGBAColor*/
};
use plotters_backend::BackendColor;

const MARVELOUS_COLOR: RGBColor = RGBColor(0x99, 0xCC, 0xFF);
const PERFECT_COLOR: RGBColor = RGBColor(0xF2, 0xCB, 0x30);
const GREAT_COLOR: RGBColor = RGBColor(0x14, 0xCC, 0x8F);
const GOOD_COLOR: RGBColor = RGBColor(0x1A, 0xB2, 0xFF);
const BAD_COLOR: RGBColor = RGBColor(0xFF, 0x1A, 0xB3);
const MISS_COLOR: RGBColor = RGBColor(0xCC, 0x29, 0x29);

const ICON_SIZE: usize = 22;

/// Takes a deviation in seconds, positive or negative, and generates the appropriate judgement
/// color
fn deviation_to_color(deviation: f32) -> RGBColor {
	match etterna::J4.classify(deviation) {
		etterna::TapJudgement::Marvelous => MARVELOUS_COLOR,
		etterna::TapJudgement::Perfect => PERFECT_COLOR,
		etterna::TapJudgement::Great => GREAT_COLOR,
		etterna::TapJudgement::Good => GOOD_COLOR,
		etterna::TapJudgement::Bad => BAD_COLOR,
		etterna::TapJudgement::Miss => MISS_COLOR,
	} //.to_rgba().mix(0.5)
}

struct IconStamper {
	icon: BitMapElement<'static, (i32, i32), plotters_bitmap::bitmap_pixel::RGBPixel>,
}

impl IconStamper {
	pub fn new(path: &str, size: u32) -> Result<Self, Box<dyn std::error::Error>> {
		use image::Pixel as _;

		let image = image::open(path)?;

		// convert RGBA->RGB, making transparent pixels black
		// plotters crate and image crate have united in their goal to make us do the most awkward
		// fucking stupid dances to do basic things
		let mut image = image.into_rgba();
		for pixel in image.pixels_mut() {
			let mut new_pixel = image::Rgba([0, 0, 0, 255]);
			new_pixel.blend(&pixel);
			*pixel = new_pixel;
		}
		let image =
			image::DynamicImage::ImageRgb8(image::DynamicImage::ImageRgba8(image).into_rgb());

		// Triangle (aka bilinear) is the fastest resize algorithm that doesn't look garbage
		let image = image.resize(size, size, image::imageops::FilterType::Triangle);

		let icon = plotters::element::BitMapElement::from(((0, 0), image));
		Ok(Self { icon })
	}

	pub fn stamp_onto<DB>(
		&mut self,
		canvas: &plotters::drawing::DrawingArea<DB, plotters::coord::Shift>,
		coord: (i32, i32),
	) -> Result<(), Box<dyn std::error::Error>>
	where
		DB: plotters::prelude::DrawingBackend,
		for<'a> &'a plotters::element::BitMapElement<'a, (i32, i32)>:
			plotters::element::PointCollection<'a, (i32, i32)>, // I don't even fucking know
	{
		self.icon.move_to(coord);
		canvas.draw(&self.icon).map_err(|e| e.to_string())?;
		Ok(())
	}
}

struct ReplayStats {
	hits: Vec<(f32, f32)>,
	mine_hit_locations: Vec<f32>,
	min_wifescore: f32,
	max_wifescore: f32,
	chart_length: f32,
}

fn gen_replay_stats(replay: &eo::Replay) -> Option<ReplayStats> {
	let notes = &replay.notes;

	let mut hits: Vec<(f32, f32)> = Vec::new();
	let mut mine_hit_locations: Vec<f32> = Vec::new();
	let mut points = 0.0;
	let mut min_wifescore = f32::INFINITY;
	let mut max_wifescore = f32::NEG_INFINITY;
	// println!("{} mine entries", notes.iter().filter(|n| n.note_type == eo::NoteType::Mine).count());
	for note in notes {
		let note_type = note.note_type?;
		match note_type {
			etterna::NoteType::Tap | etterna::NoteType::HoldHead | etterna::NoteType::Lift => {
				points += etterna::wife3(note.hit, &etterna::J4);

				// if we miss a hold head, we additionally get the hold drop penalty
				if note.hit.was_missed() && note_type == etterna::NoteType::HoldHead {
					points += etterna::Wife3::HOLD_DROP_WEIGHT;
				}

				let wifescore = points / (hits.len() + 1) as f32 * 100.0;
				hits.push((note.time as f32, wifescore));

				if wifescore < min_wifescore {
					min_wifescore = wifescore
				}
				if wifescore > max_wifescore {
					max_wifescore = wifescore
				}
			}
			etterna::NoteType::Mine => {
				points += etterna::Wife3::MINE_HIT_WEIGHT;
				mine_hit_locations.push(note.time);
			}
			etterna::NoteType::HoldTail | etterna::NoteType::Fake | etterna::NoteType::Keysound => {
			}
		}
	}

	// can't use the one-liner iterator method because float doesn't impl Ord >:(
	let mut chart_length = 0.0;
	for note in notes {
		if note.time as f32 > chart_length {
			chart_length = note.time as f32;
		}
	}

	Some(ReplayStats {
		hits,
		mine_hit_locations,
		min_wifescore,
		max_wifescore,
		chart_length,
	})
}

fn draw_mines(
	canvas: &DrawingArea<
		BitMapBackend<'_, plotters_bitmap::bitmap_pixel::RGBPixel>,
		plotters::coord::Shift,
	>,
	mine_hit_locations: &[f32],
	mut chart_time_to_x_coord: impl FnMut(f32) -> i32,
) -> Result<(), Box<dyn std::error::Error>> {
	let mut mine_hit_icon = IconStamper::new("assets/icons/mine-hit.png", 24)?;

	for &mine_hit_location in mine_hit_locations {
		mine_hit_icon.stamp_onto(
			canvas,
			(
				chart_time_to_x_coord(mine_hit_location) - ICON_SIZE as i32 / 2,
				canvas.get_pixel_range().1.end - ICON_SIZE as i32 + 2,
			),
		)?;
	}

	Ok(())
}

fn draw_hit_dots<'a, 'b>(
	replay: &eo::Replay,
	stats: &ReplayStats,
	x_range: &std::ops::Range<f32>,
	canvas: &'a DrawingArea<
		BitMapBackend<'b, plotters_bitmap::bitmap_pixel::RGBPixel>,
		plotters::coord::Shift,
	>,
) -> Result<
	ChartContext<
		'a,
		BitMapBackend<'b, plotters_bitmap::bitmap_pixel::RGBPixel>,
		Cartesian2d<plotters::coord::types::RangedCoordf32, plotters::coord::types::RangedCoordf32>,
	>,
	Box<dyn std::error::Error>,
> {
	// we leave a bit of space on the top for aesthetics, and even more space on the bottom to fit
	// the mine hit icons
	let mut dots_chart =
		ChartBuilder::on(canvas).build_cartesian_2d(x_range.clone(), -0.20..0.19f32)?;

	let draw_horizontal_line = |height: f32, color: &RGBColor| {
		let path = PathElement::new(
			vec![(0.0, height), (stats.chart_length, height)],
			ShapeStyle {
				color: color.to_rgba().mix(0.3),
				filled: false,
				stroke_width: 1,
			},
		);
		dots_chart.plotting_area().draw(&path)
	};

	draw_horizontal_line(etterna::J4.marvelous_window, &MARVELOUS_COLOR)?;
	draw_horizontal_line(-etterna::J4.marvelous_window, &MARVELOUS_COLOR)?;
	draw_horizontal_line(etterna::J4.perfect_window, &PERFECT_COLOR)?;
	draw_horizontal_line(-etterna::J4.perfect_window, &PERFECT_COLOR)?;
	draw_horizontal_line(etterna::J4.great_window, &GREAT_COLOR)?;
	draw_horizontal_line(-etterna::J4.great_window, &GREAT_COLOR)?;
	draw_horizontal_line(etterna::J4.good_window, &GOOD_COLOR)?;
	draw_horizontal_line(-etterna::J4.good_window, &GOOD_COLOR)?;
	draw_horizontal_line(etterna::J4.bad_window, &BAD_COLOR)?;
	draw_horizontal_line(-etterna::J4.bad_window, &BAD_COLOR)?;

	dots_chart.draw_series(replay.notes.iter().map(|n| {
		let x = n.time;
		let y = n.hit.deviation().unwrap_or(0.1801); // show misses as a miss instead of a bad

		EmptyElement::at((x, y))
			+ Circle::new((0, 0), 2, ShapeStyle::from(&deviation_to_color(y)).filled())
	}))?;

	Ok(dots_chart)
}

fn draw_wifescore_chart<'a, 'b>(
	canvas: &'a DrawingArea<
		BitMapBackend<'b, plotters_bitmap::bitmap_pixel::RGBPixel>,
		plotters::coord::Shift,
	>,
	x_range: &std::ops::Range<f32>,
	stats: &ReplayStats,
) -> Result<(), Box<dyn std::error::Error>> {
	let wifescore_span = stats.max_wifescore - stats.min_wifescore;
	let y_range = (stats.min_wifescore - wifescore_span / 10.0)
		..(stats.max_wifescore + wifescore_span / 10.0);
	let acc = wifescore_span < 0.5; // if the wifescore varies little, make axis labels more precise

	let mut wifescore_chart =
		ChartBuilder::on(&canvas).build_cartesian_2d(x_range.clone(), y_range.clone())?;

	wifescore_chart.draw_series(LineSeries::new(
		stats.hits.iter().copied(),
		ShapeStyle {
			color: WHITE.to_rgba(),
			filled: true,
			stroke_width: 1,
		},
	))?;

	ChartBuilder::on(&canvas)
		.y_label_area_size(if acc { 75 } else { 55 })
		.build_cartesian_2d(x_range.clone(), y_range)?
		.configure_mesh()
		.disable_mesh()
		// .disable_x_mesh()
		// .line_style_1(&WHITE.mix(0.5))
		// .line_style_2(&TRANSPARENT)
		.disable_x_axis()
		.axis_style(&WHITE.mix(0.5))
		.y_label_style(TextStyle {
			color: BackendColor {
				rgb: (255, 255, 255),
				alpha: 0.8,
			},
			pos: Pos::new(HPos::Center, VPos::Center),
			font: ("Open Sans", 18).into(),
		})
		.y_label_formatter(&|y| {
			if acc {
				format!("{:.3}%", y)
			} else {
				format!("{:.1}%", y)
			}
		})
		.y_labels(5)
		.draw()?;

	Ok(())
}

fn inner(replay: &eo::Replay, output_path: &str) -> Result<Option<()>, Box<dyn std::error::Error>> {
	let stats = match gen_replay_stats(replay) {
		Some(stats) => stats,
		None => return Ok(None),
	};

	let canvas = BitMapBackend::new(output_path, (1290, 400)).into_drawing_area();
	canvas.fill(&BLACK)?;

	let x_range = 0.0..stats.chart_length;

	let dots_chart = draw_hit_dots(&replay, &stats, &x_range, &canvas)?;

	draw_wifescore_chart(&canvas, &x_range, &stats)?;

	draw_mines(&canvas, &stats.mine_hit_locations, |time| {
		dots_chart.backend_coord(&(time, 0.0)).0
	})?;

	Ok(Some(()))
}

/// plotters did a GREAT fucking JOB of hiding their error types so that I'm **unable** to handle
/// them. For that reason, this has a String as an error type.
pub fn generate_replay_graph(
	replay: &etternaonline_api::v2::Replay,
	output_path: &str,
) -> Result<Option<()>, String> {
	// match inner(replay, output_path) {
	// 	Ok(Some(())) => Ok(()),
	// 	Ok(None) => Err(Error::InsufficientReplayData),
	// 	Err(e) => Err(Error::PlottersError(e.to_string())),
	// }
	inner(replay, output_path).map_err(|e| e.to_string())
}
