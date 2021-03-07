use plotters::{
	prelude::*,
	style::text_anchor::{HPos, Pos, VPos},
};
use plotters_backend::BackendColor;

#[derive(Debug)]
pub struct StringError(&'static str);
impl std::fmt::Display for StringError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}
impl std::error::Error for StringError {}

fn f32_max(iter: impl Iterator<Item = f32>) -> f32 {
	iter.fold(f32::NEG_INFINITY, f32::max)
}

fn inner_draw_skill_graph(
	// those two slices are guaranteed to have the same length and contain at least one item
	skill_timelines: &[etterna::SkillTimeline<&str>],
	usernames: &[&str],
	output_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
	assert_eq!(skill_timelines.len(), usernames.len());
	assert!(skill_timelines.len() >= 1);

	let label_text_style = TextStyle {
		color: BackendColor {
			rgb: (255, 255, 255),
			alpha: 0.8,
		},
		pos: Pos::new(HPos::Center, VPos::Center),
		font: ("Open Sans", 18).into(),
	};
	let mut skillset_color_map = std::collections::HashMap::new();
	skillset_color_map.insert(
		etterna::Skillset8::Overall,
		RGBColor(0xFF, 0xFF, 0xFF).to_rgba(),
	);
	skillset_color_map.insert(
		etterna::Skillset8::Stream,
		RGBColor(0x33, 0x33, 0x99).to_rgba(),
	);
	skillset_color_map.insert(
		etterna::Skillset8::Jumpstream,
		RGBColor(0x66, 0x66, 0xff).to_rgba(),
	);
	skillset_color_map.insert(
		etterna::Skillset8::Handstream,
		RGBColor(0xcc, 0x33, 0xff).to_rgba(),
	);
	skillset_color_map.insert(
		etterna::Skillset8::Stamina,
		RGBColor(0xff, 0x99, 0xcc).to_rgba(),
	);
	skillset_color_map.insert(
		etterna::Skillset8::Jackspeed,
		RGBColor(0x00, 0x99, 0x33).to_rgba(),
	);
	skillset_color_map.insert(
		etterna::Skillset8::Chordjack,
		RGBColor(0x66, 0xff, 0x66).to_rgba(),
	);
	skillset_color_map.insert(
		etterna::Skillset8::Technical,
		RGBColor(0x80, 0x80, 0x80).to_rgba(),
	);

	let root = BitMapBackend::new(output_path, (1280, 720)).into_drawing_area();
	root.fill(&RGBColor(20, 20, 20))?;

	let parsedate = |string: &str| -> chrono::Date<chrono::Utc> {
		chrono::Date::from_utc(
			chrono::NaiveDate::parse_from_str(string.trim(), "%Y-%m-%d")
				.expect("Invalid date from EO"),
			chrono::Utc,
		)
	};

	fn first_and_last<T>(arr: &[T]) -> Result<(&T, &T), StringError> {
		Ok((
			arr.first().ok_or(StringError("Empty skill timeline"))?,
			arr.last().ok_or(StringError("Empty skill timeline"))?,
		))
	}

	let (mut left_bound, mut right_bound, mut upper_bound) = {
		// UNWRAP: we check above that skill_timelines has at least one element
		let (first, last) = first_and_last(&skill_timelines.get(0).unwrap().changes)?;
		(
			first.0,
			last.0,
			f32_max(etterna::Skillset8::iter().map(|ss| last.1.get(ss))),
		)
	};
	// UNWRAP: see above
	for skill_timeline in skill_timelines.get(1..).unwrap() {
		let (first, last) = first_and_last(&skill_timeline.changes)?;

		if first.0 < left_bound {
			left_bound = first.0
		}
		if last.0 > right_bound {
			right_bound = last.0
		}
		let highest_skillset_line = f32_max(etterna::Skillset8::iter().map(|ss| last.1.get(ss)));
		if highest_skillset_line > upper_bound {
			upper_bound = highest_skillset_line
		}
	}

	let mut chart = ChartBuilder::on(&root)
		.x_label_area_size(25)
		.y_label_area_size(35)
		.margin(10)
		.build_cartesian_2d(
			parsedate(&left_bound)..parsedate(&right_bound),
			0.0..upper_bound,
		)?;

	chart
		.configure_mesh()
		.bold_line_style(&WHITE.mix(0.3))
		.light_line_style(&TRANSPARENT)
		.axis_style(&WHITE.mix(0.5))
		.x_label_style(label_text_style.clone())
		.x_label_formatter(&|dt| dt.format("%Y-%m-%d").to_string())
		.y_label_style(label_text_style.clone())
		.y_label_formatter(&|rating| format!("{:.0}", rating))
		.draw()?;

	let mut draw_timeline = |timeline: &etterna::SkillTimeline<&str>,
	                         ss: etterna::Skillset8,
	                         label: String,
	                         shape_style: ShapeStyle|
	 -> Result<(), Box<dyn std::error::Error>> {
		chart
			.draw_series(LineSeries::new(
				timeline
					.changes
					.iter()
					.zip((1..).map(|i| timeline.changes.get(i)))
					.map(|((datetime, ssr), next)| {
						let next_datetime = match next {
							Some((dt, _ssr)) => parsedate(dt),
							None => chrono::Utc::now().date(),
						};
						let ssr = ssr.get(ss);
						vec![
							// who needs allocation efficiency lolololololol
							(parsedate(datetime), ssr),
							(next_datetime, ssr),
						]
					})
					.flatten(),
				shape_style.clone(),
			))?
			.label(label)
			.legend(move |(x, y)| {
				plotters::element::Circle::new((x + 10, y), 5, shape_style.clone())
			});
		Ok(())
	};

	if skill_timelines.len() == 1 {
		// UNWRAP: see above
		let skill_timeline = &skill_timelines.get(0).unwrap();
		for ss in etterna::Skillset8::iter() {
			draw_timeline(
				&skill_timeline,
				ss,
				ss.to_string(),
				ShapeStyle {
					// UNWRAP: above we filled the hashmap with every skillset
					color: skillset_color_map.get(&ss).unwrap().clone(),
					filled: true,
					stroke_width: if ss == etterna::Skillset8::Overall {
						3
					} else {
						1
					},
				},
			)?;
		}
	} else {
		let colormap = &[
			RGBColor(0x1f, 0x77, 0xb4),
			RGBColor(0xff, 0x7f, 0x0e),
			RGBColor(0x2c, 0xa0, 0x2c),
			RGBColor(0xd6, 0x27, 0x28),
			RGBColor(0x94, 0x67, 0xbd),
			RGBColor(0x8c, 0x56, 0x4b),
			RGBColor(0xe3, 0x77, 0xc2),
			RGBColor(0x7f, 0x7f, 0x7f),
			RGBColor(0xbc, 0xbd, 0x22),
			RGBColor(0x17, 0xbe, 0xcf),
		];

		for (i, (skill_timeline, &username)) in skill_timelines.iter().zip(usernames).enumerate() {
			draw_timeline(
				&skill_timeline,
				etterna::Skillset8::Overall,
				username.to_owned(),
				ShapeStyle {
					color: colormap
						.get(i)
						.unwrap_or(&RGBColor(0xff, 0xff, 0xff))
						.to_rgba(),
					filled: true,
					stroke_width: 2,
				},
			)?;
		}
	}

	chart
		.configure_series_labels()
		.background_style(&RGBColor(10, 10, 10))
		.label_font(TextStyle {
			color: BackendColor {
				rgb: (255, 255, 255),
				alpha: 0.8,
			},
			pos: Pos::new(HPos::Left, VPos::Top),
			font: ("Open Sans", 18).into(),
		})
		.draw()?;

	Ok(())
}

pub fn draw_skill_graph(
	skill_timelines: &[etterna::SkillTimeline<&str>],
	usernames: &[&str],
	output_path: &str,
) -> Result<(), String> {
	inner_draw_skill_graph(skill_timelines, usernames, output_path).map_err(|e| e.to_string())
}
