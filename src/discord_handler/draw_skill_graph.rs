use plotters::{prelude::*, style::text_anchor::{Pos, HPos, VPos}};
use plotters_backend::BackendColor;

#[derive(Debug)]
pub struct StringError(&'static str);
impl std::fmt::Display for StringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for StringError {}

fn inner_draw_skill_graph(
	skill_timeline: &etterna::SkillTimeline<&str>,
	output_path: &str
) -> Result<(), Box<dyn std::error::Error>> {
	let label_text_style = TextStyle {
		color: BackendColor { rgb: (255, 255, 255), alpha: 0.8 },
		pos: Pos::new(HPos::Center, VPos::Center),
		font: ("Open Sans", 18).into(),
	};
	let mut skillset_color_map = std::collections::HashMap::new();
	skillset_color_map.insert(etterna::Skillset8::Overall, RGBColor(0xFF, 0xFF, 0xFF).to_rgba());
	skillset_color_map.insert(etterna::Skillset8::Stream, RGBColor(0x33, 0x33, 0x99).to_rgba());
	skillset_color_map.insert(etterna::Skillset8::Jumpstream, RGBColor(0x66, 0x66, 0xff).to_rgba());
	skillset_color_map.insert(etterna::Skillset8::Handstream, RGBColor(0xcc, 0x33, 0xff).to_rgba());
	skillset_color_map.insert(etterna::Skillset8::Stamina, RGBColor(0xff, 0x99, 0xcc).to_rgba());
	skillset_color_map.insert(etterna::Skillset8::Jackspeed, RGBColor(0x00, 0x99, 0x33).to_rgba());
	skillset_color_map.insert(etterna::Skillset8::Chordjack, RGBColor(0x66, 0xff, 0x66).to_rgba());
	skillset_color_map.insert(etterna::Skillset8::Technical, RGBColor(0x80, 0x80, 0x80).to_rgba());

	let root = BitMapBackend::new(output_path, (1280, 720)).into_drawing_area();
	root.fill(&RGBColor(20, 20, 20))?;

	let parsedate = |string: &str| -> chrono::Date<chrono::Utc> {
		chrono::Date::from_utc(
			chrono::NaiveDate::parse_from_str(string.trim(), "%Y-%m-%d")
				.expect("Invalid date from EO"),
			chrono::Utc,
		)
	};

	let first = skill_timeline.changes.first().ok_or(StringError("Empty skill timeline"))?;
	let last = skill_timeline.changes.last().ok_or(StringError("Empty skill timeline"))?;
	
	let mut chart = ChartBuilder::on(&root)
		.x_label_area_size(25)
		.y_label_area_size(35)
		.margin(10)
		.build_cartesian_2d(
			parsedate(first.0)..parsedate(last.0),
			0.0..etterna::Skillset8::iter().map(|ss| last.1.get(ss)).fold(f32::NEG_INFINITY, f32::max)
		)?;

	chart.configure_mesh()
		.bold_line_style(&WHITE.mix(0.3))
		.light_line_style(&TRANSPARENT)
		.axis_style(&WHITE.mix(0.5))
		.x_label_style(label_text_style.clone())
		.x_label_formatter(&|dt| dt.format("%Y-%m-%d").to_string())
		.y_label_style(label_text_style.clone())
		.y_label_formatter(&|rating| format!("{:.0}", rating))
		.draw()?;
	
	for ss in etterna::Skillset8::iter() {
		let color = skillset_color_map.get(&ss).unwrap();
		chart
			.draw_series(LineSeries::new(
				skill_timeline.changes.iter().zip((1..).map(|i| skill_timeline.changes.get(i)))
					.map(|((datetime, ssr), next)| {
						let next_datetime = match next {
							Some((dt, _ssr)) => parsedate(dt),
							None => chrono::Utc::now().date(),
						};
						let ssr = ssr.get_pre_070(ss);
						vec![ // who needs memory efficiency lolololololol
							(parsedate(datetime), ssr),
							(next_datetime, ssr),
						]
					})
					.flatten(),
				ShapeStyle {
					color: color.clone(),
					filled: true,
					stroke_width: if ss == etterna::Skillset8::Overall { 3 } else { 1 },
				}
			))?
			.label(ss.to_string())
			.legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], color));
	}

	chart
        .configure_series_labels()
		.background_style(&RGBColor(10, 10, 10))
		.label_font(TextStyle {
			color: BackendColor { rgb: (255, 255, 255), alpha: 0.8 },
			pos: Pos::new(HPos::Left, VPos::Top),
			font: ("Open Sans", 18).into(),
		})
        .draw()?;

	Ok(())
}

pub fn draw_skill_graph(
	skill_timeline: &etterna::SkillTimeline<&str>,
	output_path: &str
) -> Result<(), String> {
	inner_draw_skill_graph(skill_timeline, output_path).map_err(|e| e.to_string())
}