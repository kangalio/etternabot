use plotters::{
	prelude::*,
	style::{
		text_anchor::{HPos, Pos, VPos},
		RGBAColor,
	},
};
use plotters_backend::BackendColor;

fn f32_max(iter: impl Iterator<Item = f32>) -> f32 {
	iter.fold(f32::NEG_INFINITY, f32::max)
}

fn get_skillset_color(skillset: etterna::Skillset8) -> RGBColor {
	match skillset {
		etterna::Skillset8::Overall => RGBColor(0xFF, 0xFF, 0xFF),
		etterna::Skillset8::Stream => RGBColor(0x33, 0x33, 0x99),
		etterna::Skillset8::Jumpstream => RGBColor(0x66, 0x66, 0xff),
		etterna::Skillset8::Handstream => RGBColor(0xcc, 0x33, 0xff),
		etterna::Skillset8::Stamina => RGBColor(0xff, 0x99, 0xcc),
		etterna::Skillset8::Jackspeed => RGBColor(0x00, 0x99, 0x33),
		etterna::Skillset8::Chordjack => RGBColor(0x66, 0xff, 0x66),
		etterna::Skillset8::Technical => RGBColor(0x80, 0x80, 0x80),
	}
}

fn parsedate(string: &str) -> chrono::Date<chrono::Utc> {
	chrono::Date::from_utc(
		chrono::NaiveDate::parse_from_str(string.trim(), "%Y-%m-%d").expect("Invalid date from EO"),
		chrono::Utc,
	)
}

struct LineSpec<I> {
	color: RGBAColor,
	stroke_width: u32,
	label: String,
	// TODO: just use etterna::SkillTimeline<&str> as the type here
	points: I,
}

fn generic_lines_over_time(
	lines: &[LineSpec<impl IntoIterator<Item = (chrono::Date<chrono::Utc>, f32)> + Clone>],
	series_label_position: SeriesLabelPosition,
	output_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
	assert!(lines.len() >= 1);

	let label_text_style = TextStyle {
		color: BackendColor {
			rgb: (255, 255, 255),
			alpha: 0.8,
		},
		pos: Pos::new(HPos::Center, VPos::Center),
		font: ("Open Sans", 18).into(),
	};

	let root = BitMapBackend::new(output_path, (1280, 720)).into_drawing_area();
	root.fill(&RGBColor(20, 20, 20))?;

	// Find leftmost and rightmost x coordinate, as well as highest y coordinate
	let left_bound = lines
		.iter()
		.filter_map(|line| Some(line.points.clone().into_iter().next()?.0))
		.min()
		.ok_or("Empty timeline")?;
	let right_bound = lines
		.iter()
		.filter_map(|line| Some(line.points.clone().into_iter().last()?.0))
		.max()
		.ok_or("Empty timeline")?;
	let upper_bound = f32_max(
		lines
			.iter()
			.filter_map(|line| Some(line.points.clone().into_iter().last()?.1)),
	);

	let mut chart = ChartBuilder::on(&root)
		.x_label_area_size(25)
		.y_label_area_size(35)
		.margin(10)
		.build_cartesian_2d(left_bound..right_bound, 0.0..upper_bound)?;

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

	for line in lines {
		let mut line_points = line.points.clone().into_iter().peekable();
		let connection_points = std::iter::from_fn(|| {
			let (x, y) = line_points.next()?;
			let next_x = match line_points.peek() {
				Some(&(next_x, _next_y)) => next_x,
				None => chrono::Utc::now().date(),
			};

			Some(vec![(x, y), (next_x, y)])
		})
		.flatten();

		let shape_style = ShapeStyle {
			color: line.color.to_rgba(),
			stroke_width: line.stroke_width,
			filled: true,
		};
		chart
			.draw_series(LineSeries::new(connection_points, shape_style.clone()))?
			.label(line.label.clone())
			.legend(move |(x, y)| {
				plotters::element::Circle::new((x + 10, y), 5, shape_style.clone())
			});
	}

	chart
		.configure_series_labels()
		.position(series_label_position)
		.background_style(&RGBColor(10, 10, 10).mix(0.8))
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

pub fn draw_skillsets_graph(
	skill_timeline: &etterna::SkillTimeline<chrono::Date<chrono::Utc>>,
	output_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
	let mut lines = Vec::new();
	for ss in etterna::Skillset8::iter() {
		lines.push(LineSpec {
			color: get_skillset_color(ss).to_rgba(),
			stroke_width: if ss == etterna::Skillset8::Overall {
				3
			} else {
				1
			},
			label: ss.to_string(),
			points: skill_timeline
				.changes
				.iter()
				.map(move |(date, rating)| (*date, rating.get(ss))),
		});
	}

	generic_lines_over_time(&lines, SeriesLabelPosition::MiddleRight, output_path)
}

pub fn draw_user_overalls_graph(
	skill_timelines: &[etterna::SkillTimeline<chrono::Date<chrono::Utc>>],
	usernames: &[&str],
	output_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
	assert_eq!(skill_timelines.len(), usernames.len());

	const COLOR_MAP: &[RGBColor] = &[
		RGBColor(0x1F, 0x77, 0xB4),
		RGBColor(0xFF, 0x7F, 0x0E),
		RGBColor(0x2C, 0xA0, 0x2C),
		RGBColor(0xD6, 0x27, 0x28),
		RGBColor(0x94, 0x67, 0xBD),
		RGBColor(0x8C, 0x56, 0x4B),
		RGBColor(0xE3, 0x77, 0xC2),
		RGBColor(0x7F, 0x7F, 0x7F),
		RGBColor(0xBC, 0xBD, 0x22),
		RGBColor(0x17, 0xBE, 0xCF),
	];

	let mut lines = Vec::new();
	for (i, (skill_timeline, username)) in skill_timelines.iter().zip(usernames).enumerate() {
		lines.push(LineSpec {
			color: COLOR_MAP
				.get(i)
				.unwrap_or(&RGBColor(0xFF, 0xFF, 0xFF))
				.to_rgba(),
			stroke_width: 2,
			label: username.to_string(),
			points: skill_timeline
				.changes
				.iter()
				.map(move |(date, rating)| (*date, rating.overall)),
		});
	}

	generic_lines_over_time(&lines, SeriesLabelPosition::MiddleRight, output_path)
}

pub fn draw_accuracy_graph(
	full_timeline: &etterna::SkillTimeline<&str>,
	aaa_timeline: &etterna::SkillTimeline<&str>,
	aaaa_timeline: &etterna::SkillTimeline<&str>,
	output_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
	let mut lines = Vec::new();
	for (skill_timeline, name, color) in &[
		(full_timeline, "All scores", RGBColor(0xFF, 0xFF, 0xFF)),
		(aaa_timeline, "Only AAA+", RGBColor(0xEE, 0xBB, 0x00)),
		(aaaa_timeline, "Only AAAA+", RGBColor(0x66, 0xCC, 0xFF)),
	] {
		lines.push(LineSpec {
			color: color.to_rgba(),
			stroke_width: 2,
			label: name.to_string(),
			points: skill_timeline
				.changes
				.iter()
				.map(move |(date, rating)| (parsedate(date), rating.overall)),
		});
	}

	generic_lines_over_time(&lines, SeriesLabelPosition::MiddleRight, output_path)
}

pub struct ScoreGraphUser {
	pub sub_aa_timeline: Option<Vec<(chrono::Date<chrono::Utc>, u32)>>,
	pub aa_timeline: Vec<(chrono::Date<chrono::Utc>, u32)>,
	pub aaa_timeline: Vec<(chrono::Date<chrono::Utc>, u32)>,
	pub aaaa_timeline: Vec<(chrono::Date<chrono::Utc>, u32)>,
	pub username: String,
}

fn lerp_color(a: RGBColor, b: RGBColor, t: f32) -> RGBColor {
	RGBColor(
		(a.0 as f32 + (b.0 as f32 - a.0 as f32) * t) as u8,
		(a.1 as f32 + (b.1 as f32 - a.1 as f32) * t) as u8,
		(a.2 as f32 + (b.2 as f32 - a.2 as f32) * t) as u8,
	)
}

pub fn draw_score_graph(
	users: &[ScoreGraphUser],
	output_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
	assert!(!users.is_empty());

	const COLOR_MAP: &[RGBColor] = &[
		RGBColor(0x1F, 0x77, 0xB4),
		RGBColor(0xFF, 0x7F, 0x0E),
		RGBColor(0x2C, 0xA0, 0x2C),
		RGBColor(0xD6, 0x27, 0x28),
		RGBColor(0x94, 0x67, 0xBD),
		RGBColor(0x8C, 0x56, 0x4B),
		RGBColor(0xE3, 0x77, 0xC2),
		RGBColor(0x7F, 0x7F, 0x7F),
		RGBColor(0xBC, 0xBD, 0x22),
		RGBColor(0x17, 0xBE, 0xCF),
	];

	let mut lines = Vec::new();
	for (user_i, user) in users.iter().enumerate() {
		let user_timelines = &[
			(
				user.sub_aa_timeline.as_ref(),
				"# of sub-AAs",
				RGBColor(0xDA, 0x57, 0x57),
				(0.0, 0.4),
			),
			(
				Some(&user.aa_timeline),
				"# of AAs",
				RGBColor(0x66, 0xCC, 0x66),
				(0.0, 0.0),
			),
			(
				Some(&user.aaa_timeline),
				"# of AAAs",
				RGBColor(0xEE, 0xBB, 0x00),
				(0.2, 0.0),
			),
			(
				Some(&user.aaaa_timeline),
				"# of AAAAs",
				RGBColor(0x66, 0xCC, 0xFF),
				(0.4, 0.0),
			),
		];
		for &(timeline, base_name, grade_color, (lightness, darkness)) in user_timelines {
			let timeline = match timeline {
				Some(x) => x,
				None => continue,
			};

			let name = if users.len() == 1 {
				base_name.to_owned()
			} else {
				format!("{}: {}", user.username, base_name)
			};

			let color = if users.len() == 1 {
				grade_color
			} else {
				let base_color = *COLOR_MAP.get(user_i).unwrap_or(&RGBColor(0x00, 0xFF, 0x00));
				let (white, black) = (RGBColor(0xFF, 0xFF, 0xFF), RGBColor(0, 0, 0));
				lerp_color(lerp_color(base_color, white, lightness), black, darkness)
			};

			lines.push(LineSpec {
				color: color.to_rgba(),
				stroke_width: 2,
				label: name.to_string(),
				points: timeline.iter().map(|&(date, amount)| (date, amount as f32)),
			});
		}
	}

	generic_lines_over_time(&lines, SeriesLabelPosition::MiddleLeft, output_path)
}
