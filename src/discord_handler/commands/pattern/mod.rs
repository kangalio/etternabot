mod pattern_draw;
pub use pattern_draw::{Error as PatternError, Noteskin};

use super::State;
use crate::{serenity, Error};

const CMD_SCROLLSET_HELP: &str = "Call this command with `+scrollset [down/up]`";

pub struct NoteskinProvider {
	dbz: pattern_draw::Noteskin,
	lambda: pattern_draw::Noteskin,
	wafles: pattern_draw::Noteskin,
	delta_note: pattern_draw::Noteskin,
	sbz: pattern_draw::Noteskin,
	mbz: pattern_draw::Noteskin,
	eo_baner: pattern_draw::Noteskin,
	rustmania: pattern_draw::Noteskin,
}

impl NoteskinProvider {
	pub fn load() -> Result<Self, PatternError> {
		Ok(Self {
			dbz: Noteskin::read_ldur_with_6k(
				64,
				"assets/noteskin/dbz-notes.png",
				"assets/noteskin/dbz-receptor.png",
				"assets/noteskin/dbz-mine.png",
			)?,
			delta_note: Noteskin::read_pump(
				64,
				"assets/noteskin/deltanote-center-notes.png",
				"assets/noteskin/deltanote-center-receptor.png",
				"assets/noteskin/deltanote-corner-notes.png",
				"assets/noteskin/deltanote-corner-receptor.png",
				"assets/noteskin/deltanote-mine.png",
			)?,
			sbz: Noteskin::read_bar(
				64,
				"assets/noteskin/sbz-notes.png",
				"assets/noteskin/sbz-receptor.png",
				"assets/noteskin/dbz-mine.png",
			)?,
			mbz: Noteskin::read_bar(
				64,
				"assets/noteskin/mbz-notes.png",
				"assets/noteskin/mbz-receptor.png",
				"assets/noteskin/dbz-mine.png",
			)?,
			lambda: {
				let mut lambda = Noteskin::read_ldur_with_6k(
					128,
					"assets/noteskin/lambda-notes.png",
					"assets/noteskin/lambda-receptor.png",
					"assets/noteskin/lambda-mine.png",
				)?;
				lambda.resize_sprites(64);
				lambda
			},
			wafles: Noteskin::read_ldur_with_6k(
				64,
				"assets/noteskin/wafles-notes.png",
				"assets/noteskin/wafles-receptor.png",
				"assets/noteskin/wafles-mine.png",
			)?,
			eo_baner: Noteskin::read_ldur(
				120,
				"assets/noteskin/eobaner-note-left.png",
				"assets/noteskin/eobaner-receptor-left.png",
				"assets/noteskin/eobaner-note-down.png",
				"assets/noteskin/eobaner-receptor-down.png",
				"assets/noteskin/eobaner-note-up.png",
				"assets/noteskin/eobaner-receptor-up.png",
				"assets/noteskin/eobaner-note-right.png",
				"assets/noteskin/eobaner-receptor-right.png",
				"assets/noteskin/eobaner-mine.png",
			)?,
			rustmania: {
				let mut rustmania = Noteskin::read_ldur_with_6k(
					224,
					"assets/noteskin/rustmania-notes.png",
					"assets/noteskin/rustmania-receptor.png",
					"assets/noteskin/rustmania-mine.png",
				)?;
				rustmania.turn_sprites_upside_down(); // I made an oopsie in gimp
				rustmania
			},
		})
	}
}

pub fn pattern(
	state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
	args: &str,
) -> Result<(), Error> {
	let mut noteskin_override = None;
	let mut keymode_override = None;
	let mut snap = etterna::Snap::_16th.into();
	let mut vertical_spacing_multiplier = 1.0;
	let mut scroll_direction = state
		.lock_data()
		.scroll(msg.author.id.0)
		.unwrap_or(etterna::ScrollDirection::Upscroll);
	let mut segments = Vec::new();

	let extract_snap = |string: &str, user_intended: &mut bool| {
		const ENDINGS: &[&str] = &["st", "sts", "nd", "nds", "rd", "rds", "th", "ths"];

		let characters_to_truncate = ENDINGS
			.iter()
			.find(|&ending| string.ends_with(ending))?
			.len();
		// UNWRAP: we're only removing up to the string length, so we can't go out-of-bounds
		let string_without_ending = string
			.get(..(string.len() - characters_to_truncate))
			.unwrap();
		let snap: u32 = string_without_ending.parse().ok()?;
		*user_intended = true;
		pattern_draw::FractionalSnap::from_snap_number(snap)
	};
	let extract_noteskin = |string: &str, _user_intended: &mut bool| {
		// make lowercase and remove all special characters
		let mut normalized_noteskin_name = string.to_ascii_lowercase();
		normalized_noteskin_name.retain(|c| c.is_alphanumeric());

		match normalized_noteskin_name.as_str() {
			"dbz" | "dividebyzero" => Some(&state.noteskin_provider.dbz),
			"wafles" | "wafles3" => Some(&state.noteskin_provider.wafles),
			"default" | "lambda" => Some(&state.noteskin_provider.lambda),
			"deltanote" | "delta" => Some(&state.noteskin_provider.delta_note),
			"sbz" | "subtractbyzero" => Some(&state.noteskin_provider.sbz),
			"mbz" | "multiplybyzero" => Some(&state.noteskin_provider.mbz),
			"eobaner" => Some(&state.noteskin_provider.eo_baner),
			"rustmania" => Some(&state.noteskin_provider.rustmania),
			_ => None,
		}
	};
	let extract_vertical_spacing_multiplier = |string: &str, user_intended: &mut bool| {
		if !string.ends_with('x') {
			return None;
		};
		// UNWRAP: at this point the string must have 'x' at the end so we can safely strip one char
		let vertical_spacing_multiplier: f32 =
			string.get(..(string.len() - 1)).unwrap().parse().ok()?;
		*user_intended = true;
		if vertical_spacing_multiplier > 0.0 {
			Some(vertical_spacing_multiplier)
		} else {
			None
		}
	};
	let extract_scroll_direction =
		|string: &str, _user_intended: &mut bool| match string.to_lowercase().as_str() {
			"up" => Some(etterna::ScrollDirection::Upscroll),
			"down" | "reverse" => Some(etterna::ScrollDirection::Downscroll),
			_ => None,
		};
	let extract_keymode = |string: &str, user_intended: &mut bool| {
		if !(string.ends_with('k') || string.ends_with('K')) {
			return None;
		}

		// UNWRAP: at this point the string must have 'k' at the end so we can safely strip one char
		let keymode: u32 = string.get(..(string.len() - 1)).unwrap().parse().ok()?;
		*user_intended = true;
		if keymode > 0 {
			Some(keymode)
		} else {
			None
		}
	};

	let mut pattern_buffer = String::new();
	for arg in args.split_whitespace() {
		let mut did_user_intend = false;
		if let Some(new_snap) = extract_snap(arg, &mut did_user_intend) {
			if pattern_buffer.len() > 0 {
				segments.push((pattern_draw::parse_pattern(&pattern_buffer)?, snap));
				pattern_buffer.clear();
			}
			snap = new_snap;
			continue;
		}
		if did_user_intend {
			msg.channel_id
				.say(&ctx.http, format!("\"{}\" is not a valid snap", arg))?;
		}

		let mut did_user_intend = false;
		if let Some(noteskin) = extract_noteskin(arg, &mut did_user_intend) {
			noteskin_override = Some(noteskin);
			continue;
		}
		if did_user_intend {
			msg.channel_id.say(
				&ctx.http,
				format!("\"{}\" is not a valid noteskin name", arg),
			)?;
		}

		let mut did_user_intend = false;
		if let Some(vertical_spacing_multiplier_override) =
			extract_vertical_spacing_multiplier(arg, &mut did_user_intend)
		{
			vertical_spacing_multiplier = vertical_spacing_multiplier_override;
			continue;
		}
		if did_user_intend {
			msg.channel_id
				.say(&ctx.http, format!("\"{}\" is not a valid zoom option", arg))?;
		}

		let mut did_user_intend = false;
		if let Some(scroll_direction_override) = extract_scroll_direction(arg, &mut did_user_intend)
		{
			scroll_direction = scroll_direction_override;
			continue;
		}
		if did_user_intend {
			msg.channel_id.say(
				&ctx.http,
				format!("\"{}\" is not a valid scroll direction", arg),
			)?;
		}

		let mut did_user_intend = false;
		if let Some(keymode) = extract_keymode(arg, &mut did_user_intend) {
			keymode_override = Some(keymode);
			continue;
		}
		if did_user_intend {
			msg.channel_id
				.say(&ctx.http, format!("\"{}\" is not a valid keymode", arg))?;
		}

		// if nothing matched, this is just an ordinary part of the pattern
		pattern_buffer += arg;
	}
	if pattern_buffer.len() > 0 {
		segments.push((pattern_draw::parse_pattern(&pattern_buffer)?, snap));
		pattern_buffer.clear();
	}

	let keymode = if let Some(keymode) = keymode_override {
		keymode
	} else {
		let highest_lane = segments
			.iter()
			.flat_map(|(pattern, _)| &pattern.rows)
			// if the user entered `+pattern ldr`, was the highest column 3, or 4? remember, the
			// meaning of `r` depends on keymode, but we don't know the keymode yet. I've
			// decided to assume 4k in the fallback case
			.filter_map(|row| {
				row.iter()
					.map(|(lane, _note_type)| lane.column_number_with_keymode(4))
					.max()
			})
			.max()
			.ok_or(PatternError::EmptyPattern)?;
		let keymode = (highest_lane + 1) as u32;
		keymode.max(4) // clamp keymode to a minimum of 4k. yes, 3k exists, but it's so niche that even if only three lanes are populated, the pattern is probably meant to be 4k
	};

	let noteskin = if let Some(noteskin) = noteskin_override {
		&noteskin
	} else {
		// choose a default noteskin
		match keymode {
			3 | 4 | 6 | 8 => &state.noteskin_provider.dbz,
			5 | 10 => &state.noteskin_provider.delta_note,
			7 | 9 => &state.noteskin_provider.sbz,
			_ => &state.noteskin_provider.sbz, // fallback
		}
	};

	let generated_pattern = pattern_draw::draw_pattern(pattern_draw::PatternRecipe {
		noteskin,
		scroll_direction,
		keymode: keymode as usize, /* I thought I had changedit to u32 in pattern_draw???? */
		vertical_spacing_multiplier,
		pattern: &segments,
		max_image_dimensions: (5000, 10000),
		max_sprites: 1000,
	})?;

	let mut img_bytes = Vec::with_capacity(1_000_000); // preallocate 1 MB for the img
	image::DynamicImage::ImageRgba8(generated_pattern)
		.write_to(&mut img_bytes, image::ImageOutputFormat::Png)
		.map_err(pattern_draw::Error::ImageError)?;

	// Send the image into the channel where the summoning message comes from
	msg.channel_id
		.send_files(&ctx.http, vec![(img_bytes.as_slice(), "output.png")], |m| m)?;

	Ok(())
}

pub fn scrollset(
	state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
	args: &str,
) -> Result<(), Error> {
	let scroll = match &args.to_lowercase() as &str {
		"down" | "downscroll" => etterna::ScrollDirection::Downscroll,
		"up" | "upscroll" => etterna::ScrollDirection::Upscroll,
		"" => {
			msg.channel_id.say(&ctx.http, CMD_SCROLLSET_HELP)?;
			return Ok(());
		}
		_ => {
			msg.channel_id
				.say(&ctx.http, format!("No such scroll '{}'", args))?;
			return Ok(());
		}
	};
	state.lock_data().set_scroll(msg.author.id.0, scroll);
	msg.channel_id
		.say(&ctx.http, &format!("Your scroll type is now {:?}", scroll))?;

	Ok(())
}