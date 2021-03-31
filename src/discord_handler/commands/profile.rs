use std::borrow::Cow;

use super::State;
use crate::{serenity, Error};

const CMD_COMPARE_HELP: &str = "Call this command with `+compare OTHER_USER` or `+compare USER OTHER_USER`. Add `expanded` at the end to see a graphic";
const CMD_USERSET_HELP: &str = "Call this command with `+userset YOUR_EO_USERNAME`";
const CMD_RIVALSET_HELP: &str = "Call this command with `+rivalset YOUR_EO_USERNAME`";

fn country_code_to_flag_emoji(country_code: &str) -> Option<String> {
	if country_code.chars().any(|c| !c.is_alphabetic()) {
		return None;
	}

	let regional_indicator_value_offset = '🇦' as u32 - 'a' as u32;
	country_code
		.chars()
		.map(|c| {
			std::char::from_u32(c.to_ascii_lowercase() as u32 + regional_indicator_value_offset)
		})
		.collect()
}

/// Returns a string that may be shorter than `max_length`, but never longer
/// (measured in chars, not in bytes!)
fn gen_unicode_block_bar(max_length: usize, proportion: f32) -> String {
	// index x = x 8ths of a full block
	const BLOCK_CHARS: [char; 9] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];

	let num_possible_steps = max_length * 8;
	let step = (proportion * num_possible_steps as f32) as usize;

	let num_full_blocks = step / 8;
	let type_of_last_block = step % 8;

	let mut string = String::with_capacity(max_length);
	for _ in 0..num_full_blocks {
		string.push(BLOCK_CHARS[8]);
	}
	// UNWRAP: due to the modulo the index is guaranteed to be in bounds
	string.push(*BLOCK_CHARS.get(type_of_last_block).unwrap());

	if let Some((truncation_point, _)) = string.char_indices().nth(max_length) {
		string.truncate(truncation_point);
	}

	string
}

/// Maps a value from src_range to dest_range. The value doesn't need to be inside src_range
///
/// ```rust
/// assert_eq!(map_range(15.0, 10.0..20.0, 3.0..4.0), 3.5);
/// assert_eq!(map_range(15.0, 10.0..20.0, -1.0, -3.0), -2.0);
/// assert_eq!(map_range(30.0, 10.0..20.0, -1.0, -3.0), -5.0);
/// ```
fn rescale(value: f32, src_range: std::ops::Range<f32>, dest_range: std::ops::Range<f32>) -> f32 {
	let proportion = (value - src_range.start) / (src_range.end - src_range.start);
	dest_range.start + proportion * (dest_range.end - dest_range.start)
}

fn profile_compare(
	state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
	me: &str,
	you: &str,
	expanded: bool,
) -> Result<(), Error> {
	let me = state.v2()?.user_details(me)?;
	let you = state.v2()?.user_details(you)?;

	let my_rating = &me.rating;
	let your_rating = &you.rating;

	let mut string = "```Prolog\n".to_owned();
	for skillset in etterna::Skillset8::iter() {
		string += &format!(
			"{: >10}:   {: >5.2}  {}  {: >5.2}   {:+.2}\n",
			skillset.to_string(), // to_string, or the padding won't work
			my_rating.get(skillset),
			if (my_rating.get(skillset) - your_rating.get(skillset)).abs() < f32::EPSILON {
				"="
			} else if my_rating.get(skillset) > your_rating.get(skillset) {
				">"
			} else {
				"<"
			},
			your_rating.get(skillset),
			my_rating.get(skillset) - your_rating.get(skillset),
		);
	}
	string += "```";

	let (mut min_ss_rating, mut max_ss_rating) = (f32::INFINITY, f32::NEG_INFINITY);
	for ss in etterna::Skillset8::iter() {
		let my_rating = my_rating.get(ss);
		let your_rating = your_rating.get(ss);
		if my_rating < min_ss_rating {
			min_ss_rating = my_rating;
		}
		if your_rating < min_ss_rating {
			min_ss_rating = your_rating;
		}
		if my_rating > max_ss_rating {
			max_ss_rating = my_rating;
		}
		if your_rating > max_ss_rating {
			max_ss_rating = your_rating;
		}
	}

	let bar_graph_block = if expanded {
		let mut bar_graph_block = "```prolog\n".to_owned();
		for skillset in etterna::Skillset8::iter() {
			let my_rating = my_rating.get(skillset);
			let your_rating = your_rating.get(skillset);
			bar_graph_block += &format!(
				"{: >10}:   \"░▒▓{}\"\n              “░▒▓{}“\n\n",
				skillset.to_string(), // to_string, or the padding won't work
				gen_unicode_block_bar(
					18,
					rescale(my_rating, min_ss_rating..max_ss_rating, 0.0..1.0)
				),
				gen_unicode_block_bar(
					18,
					rescale(your_rating, min_ss_rating..max_ss_rating, 0.0..1.0)
				),
			)
		}
		bar_graph_block += "```";
		Some(bar_graph_block)
	} else {
		None
	};

	msg.channel_id.send_message(&ctx.http, |m| {
		m.embed(|e| {
			e.color(crate::ETTERNA_COLOR)
				.title(format!(
					"{} {} vs. {} {}",
					country_code_to_flag_emoji(&me.country_code).unwrap_or_else(|| "❓".into()),
					me.username,
					you.username,
					country_code_to_flag_emoji(&you.country_code).unwrap_or_else(|| "❓".into()),
				))
				.description(string);

			if let Some(bar_graph_block) = bar_graph_block {
				e.field(
					format!("Above is {}, below is {}", me.username, you.username),
					bar_graph_block,
					false,
				);
			}

			e
		})
	})?;

	Ok(())
}

pub fn rival(
	state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
	args: &str,
) -> Result<(), Error> {
	let me = &state.get_eo_username(ctx, msg)?;
	let you = match state.lock_data().rival(msg.author.id.0) {
		Some(rival) => rival.to_owned(),
		None => {
			msg.channel_id
				.say(&ctx.http, "Set your rival first with `+rivalset USERNAME`")?;
			return Ok(());
		}
	};

	let expanded = args == "expanded";

	profile_compare(state, ctx, msg, me, &you, expanded)
}

pub fn compare(
	state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
	args: &str,
) -> Result<(), Error> {
	let args: Vec<&str> = args.split_whitespace().collect();

	let (me, you, expanded) = match *args.as_slice() {
		[you] => (state.get_eo_username(ctx, msg)?, you, false),
		[you, "expanded"] => (state.get_eo_username(ctx, msg)?, you, true),
		[me, you] => (me.to_owned(), you, false),
		[me, you, "expanded"] => (me.to_owned(), you, true),
		_ => {
			msg.channel_id.say(&ctx.http, CMD_COMPARE_HELP)?;
			return Ok(());
		}
	};

	profile_compare(state, ctx, msg, &me, you, expanded)
}

pub fn userset(
	state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
	args: &str,
) -> Result<(), Error> {
	if args.is_empty() {
		msg.channel_id.say(&ctx.http, CMD_USERSET_HELP)?;
		return Ok(());
	}

	let new_user_entry = super::config::UserRegistryEntry {
		discord_id: msg.author.id.0,
		discord_username: msg.author.name.to_owned(),
		eo_id: state.web_session.user_details(args)?.user_id,
		eo_username: args.to_owned(),
		last_known_num_scores: None,
		last_rating: None,
	};

	let mut data = state.lock_data();
	match data
		.user_registry
		.iter_mut()
		.find(|u| u.discord_id == msg.author.id.0)
	{
		Some(existing_user_entry) => {
			msg.channel_id.say(
				&ctx.http,
				format!(
					"Successfully updated username from `{}` to `{}`",
					existing_user_entry.eo_username, new_user_entry.eo_username,
				),
			)?;

			*existing_user_entry = new_user_entry;
		}
		None => {
			msg.channel_id.say(
				&ctx.http,
				format!("Successfully set username to `{}`", args),
			)?;

			data.user_registry.push(new_user_entry);
		}
	};

	Ok(())
}

pub fn rivalset(
	state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
	args: &str,
) -> Result<(), Error> {
	if args.is_empty() {
		msg.channel_id.say(&ctx.http, CMD_RIVALSET_HELP)?;
		return Ok(());
	}
	if let Err(etternaonline_api::Error::UserNotFound) = state.v2()?.user_details(args) {
		msg.channel_id
			.say(&ctx.http, &format!("User `{}` doesn't exist", args))?;
		return Ok(());
	}

	let response = match state
		.lock_data()
		.set_rival(msg.author.id.0, args.to_owned())
	{
		Some(old_rival) => format!(
			"Successfully updated your rival from `{}` to `{}`",
			old_rival, args,
		),
		None => format!("Successfully set your rival to `{}`", args),
	};
	msg.channel_id.say(&ctx.http, &response)?;

	Ok(())
}

fn truncate_text_maybe(text_body: &str, max_length: usize) -> Cow<'_, str> {
	let truncation_msg = "...";

	// check the char limit first, because otherwise we could produce a too large message
	if text_body.len() + truncation_msg.len() > max_length {
		// This is how long the text body may be at max to conform to Discord's limit
		let available_space = max_length - truncation_msg.len();

		let mut cut_off_point = available_space;
		while !text_body.is_char_boundary(cut_off_point) {
			cut_off_point -= 1;
		}

		Cow::Owned(format!("{}{}", &text_body[..cut_off_point], truncation_msg))
	} else {
		Cow::Borrowed(text_body)
	}
}

pub fn profile(
	state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
	text: &str,
) -> Result<(), Error> {
	let (eo_username, overwrite_prev_ratings) = if text.is_empty() {
		(state.get_eo_username(ctx, msg)?, true)
	} else {
		(text.to_owned(), false)
	};

	let details = state.v2()?.user_details(&eo_username)?;
	let ranks = state.v2()?.user_ranks_per_skillset(&eo_username)?;

	let mut title = eo_username.to_owned();
	if details.is_moderator {
		title += " (Mod)";
	}
	if details.is_patreon {
		title += " (Patron)";
	}

	let (mut min_ss_rating, mut max_ss_rating) = (f32::INFINITY, f32::NEG_INFINITY);
	for ss in etterna::Skillset8::iter() {
		let ss_rating = details.rating.get(ss);
		if ss_rating < min_ss_rating {
			min_ss_rating = ss_rating;
		}
		if ss_rating > max_ss_rating {
			max_ss_rating = ss_rating;
		}
	}

	let mut data = state.lock_data();
	// None if user is not in registry, None(None) if user is in registry but no prev rating
	let previous_ratings = data
		.user_registry
		.iter_mut()
		.find(|entry| entry.eo_username.eq_ignore_ascii_case(&eo_username))
		.map(|entry| &mut entry.last_rating);

	let mut rating_string = "```prolog\n".to_owned();
	for skillset in etterna::Skillset8::iter() {
		match &previous_ratings {
			Some(Some(prev)) => {
				rating_string += &format!(
					"{: >10}:   {: >5.2} ({: >+4.2})  #{: <4}\n",
					skillset.to_string(),
					details.rating.get(skillset),
					details.rating.get(skillset) - prev.get(skillset),
					ranks.get(skillset),
				)
			}
			Some(None) | None => {
				rating_string += &format!(
					"{: >10}:   {: >5.2}  #{: <4}\n",
					skillset.to_string(),
					details.rating.get(skillset),
					ranks.get(skillset),
				)
			}
		}
	}
	rating_string += "```";

	if overwrite_prev_ratings {
		// TODO: could create new entry if doesn't already exist to store ratings
		if let Some(previous_ratings) = previous_ratings {
			*previous_ratings = Some(details.rating.clone());
		}
	}

	msg.channel_id.send_message(&ctx.http, |m| {
		m.embed(|embed| {
			embed
				.description(rating_string)
				.author(|a| {
					a.name(&title)
						.url(format!(
							"https://etternaonline.com/user/profile/{}",
							&eo_username
						))
						.icon_url(format!(
							"https://etternaonline.com/img/flags/{}.png",
							&details.country_code
						))
				})
				.thumbnail(format!(
					"https://etternaonline.com/avatars/{}",
					&details.avatar_url
				))
				.color(crate::ETTERNA_COLOR);
			if let Some(modifiers) = &details.default_modifiers {
				embed.field("Default modifiers:", modifiers, false);
			}
			if !details.about_me.is_empty() {
				embed.field(
					format!("About {}:", eo_username),
					truncate_text_maybe(&html2md::parse_html(&details.about_me), 1024),
					false,
				);
			}

			embed
		})
	})?;

	Ok(())
}
