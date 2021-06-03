use super::Context;
use crate::Error;
use std::borrow::Cow;

/// Returns a question mark emoji on invalid country code
fn country_code_to_flag_emoji(country_code: &str) -> String {
	fn inner(country_code: &str) -> Option<String> {
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
	inner(country_code).unwrap_or_else(|| "❓".into())
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

async fn profile_compare(
	ctx: Context<'_>,
	me: &str,
	you: &str,
	expanded: bool,
) -> Result<(), Error> {
	let me = ctx.data().v2().await?.user_details(me).await?;
	let you = ctx.data().v2().await?.user_details(you).await?;

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

	poise::send_reply(ctx, |m| {
		m.embed(|e| {
			e.color(crate::ETTERNA_COLOR)
				.title(format!(
					"{} {} vs. {} {}",
					country_code_to_flag_emoji(&me.country_code),
					me.username,
					you.username,
					country_code_to_flag_emoji(&you.country_code),
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
	})
	.await?;

	Ok(())
}

/// Compare your skillsets against your rival
#[poise::command(track_edits, slash_command)]
pub async fn rival(
	ctx: Context<'_>,
	#[description = "Show a bar chart of individual skillsets"]
	#[flag]
	expanded: bool,
) -> Result<(), Error> {
	let me = &ctx.data().get_eo_username(ctx.author()).await?;

	let rival = ctx
		.data()
		.lock_data()
		.rival(ctx.author().id.0)
		.map(|x| x.to_owned());
	let you = match rival {
		Some(rival) => rival,
		None => {
			poise::say_reply(ctx, "Set your rival first with `+rivalset USERNAME`".into()).await?;
			return Ok(());
		}
	};

	profile_compare(ctx, me, &you, expanded).await
}

/// Compare two users' skillsets.
///
/// Call this command with `+compare OTHER_USER` or `+compare USER OTHER_USER`. Add `expanded` at the end to see a graphic
#[poise::command(track_edits, slash_command)]
pub async fn compare(
	ctx: Context<'_>,
	#[description = "User on the left side of the comparison"]
	#[lazy]
	left: Option<String>,
	#[description = "User on the right side of the comparison"] right: String,
	#[description = "Show a bar chart of individual skillsets"]
	#[flag]
	expanded: bool,
) -> Result<(), Error> {
	let left = match left {
		Some(x) => x,
		None => ctx.data().get_eo_username(ctx.author()).await?,
	};

	profile_compare(ctx, &left, &right, expanded).await
}

/// Save your EtternaOnline username in the bot
///
/// Call this command with `+userset YOUR_EO_USERNAME`
#[poise::command(track_edits, slash_command)]
pub async fn userset(
	ctx: Context<'_>,
	#[description = "Your EtternaOnline username"] username: String,
) -> Result<(), Error> {
	let new_user_entry = super::config::UserRegistryEntry {
		discord_id: ctx.author().id.0,
		discord_username: ctx.author().name.to_owned(),
		eo_id: ctx
			.data()
			.web_session
			.user_details(&username)
			.await?
			.user_id,
		eo_username: username.to_owned(),
		last_known_num_scores: None,
		last_rating: None,
	};
	println!("a");

	let old_eo_username;
	{
		let mut data = ctx.data().lock_data();
		match data
			.user_registry
			.iter_mut()
			.find(|u| u.discord_id == ctx.author().id.0)
		{
			Some(existing_user_entry) => {
				old_eo_username = Some(existing_user_entry.eo_username.clone());
				*existing_user_entry = new_user_entry;
			}
			None => {
				old_eo_username = None;
				data.user_registry.push(new_user_entry);
			}
		}
	}

	let response = match old_eo_username {
		Some(old) => format!(
			"Successfully updated username from `{}` to `{}`",
			old, username,
		),
		None => format!("Successfully set username to `{}`", username),
	};
	println!("b");
	poise::say_reply(ctx, response).await?;

	Ok(())
}

/// Set a rival to compete against!
///
/// Call this command with `+rivalset YOUR_EO_USERNAME`
#[poise::command(track_edits, slash_command)]
pub async fn rivalset(
	ctx: Context<'_>,
	#[description = "EtternaOnline username of your new rival"] rival: String,
) -> Result<(), Error> {
	if ctx.data().v2().await?.user_details(&rival).await.is_err() {
		poise::say_reply(ctx, format!("User `{}` doesn't exist", rival)).await?;
		return Ok(());
	}

	let response = match ctx
		.data()
		.lock_data()
		.set_rival(ctx.author().id.0, rival.to_owned())
	{
		Some(old_rival) => format!(
			"Successfully updated your rival from `{}` to `{}`",
			old_rival, rival,
		),
		None => format!("Successfully set your rival to `{}`", rival),
	};
	poise::say_reply(ctx, response).await?;

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

/// Display your skillsets and your improvements since last time
#[poise::command(aliases("advprof"), track_edits, slash_command)]
pub async fn profile(
	ctx: Context<'_>,
	#[description = "EtternaOnline username. If not specified, shows your stats"] // dummy
	eo_username: Option<String>,
) -> Result<(), Error> {
	let (eo_username, overwrite_prev_ratings) = match eo_username {
		Some(eo_username) => (eo_username, false),
		None => (ctx.data().get_eo_username(ctx.author()).await?, true),
	};

	let details = ctx.data().v2().await?.user_details(&eo_username).await?;
	let ranks = ctx
		.data()
		.v2()
		.await?
		.user_ranks_per_skillset(&eo_username)
		.await?;

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

	let rating_string = {
		let mut data = ctx.data().lock_data();
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

		rating_string
	};

	poise::send_reply(ctx, |m| {
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
	})
	.await?;

	Ok(())
}

/// Retrieve leaderboard entries directly above and below the current user.
///
/// Call this command with `+aroundme [USERNAME] [SKILLSET] [AMOUNT]
#[poise::command(slash_command, track_edits)]
pub async fn aroundme(
	ctx: Context<'_>,
	#[lazy]
	#[description = "EtternaOnline username"]
	username: Option<String>,
	#[description = "Skillset to sort by"] skillset: Option<poise::Wrapper<etterna::Skillset8>>,
	#[description = "How many entries to fetch above and below"] num_entries: Option<u32>,
) -> Result<(), Error> {
	let username = match username {
		Some(x) => x.to_owned(),
		None => ctx.data().get_eo_username(ctx.author()).await?,
	};

	let skillset = match skillset {
		Some(x) => x.0,
		None => etterna::Skillset8::Overall,
	};

	let num_entries = num_entries.unwrap_or(7);

	let ranks = ctx
		.data()
		.v2()
		.await?
		.user_ranks_per_skillset(&username)
		.await?;
	let rank = ranks.get(skillset);

	let self_index = rank - 1; // E.g. first player in leaderboard has rank 1 but index 0;
	let entries = ctx
		.data()
		.web_session
		.leaderboard(
			self_index.saturating_sub(num_entries)..=(self_index + num_entries),
			etternaonline_api::web::LeaderboardSortBy::Rating(skillset),
			etternaonline_api::web::SortDirection::Descending,
		)
		.await?;

	// Detect if user doesn't exist (EO doesn't actually tell us this, it just returns garbage
	// results)
	let all_ones = etterna::UserRank {
		overall: 1,
		stream: 1,
		jumpstream: 1,
		handstream: 1,
		stamina: 1,
		jackspeed: 1,
		chordjack: 1,
		technical: 1,
	};
	let username_present_in_results = entries
		.iter()
		.any(|entry| entry.username.eq_ignore_ascii_case(&username));
	if ranks == all_ones && !username_present_in_results {
		return Err(etternaonline_api::Error::UserNotFound.into());
	}

	let self_entry = entries
		.iter()
		.find(|entry| entry.username.eq_ignore_ascii_case(&username))
		.or_else(|| entries.iter().find(|entry| entry.rank == rank)) // fallback 1
		.or_else(|| entries.get(0)) // fallback 2
		.ok_or("Error when retrieving leaderboard entries")?; // welp we did everything we could

	let mut output = String::from("```c\n");
	for entry in &entries {
		let is_self = std::ptr::eq(self_entry, entry);

		let flag_emoji = match &entry.country {
			Some(country) => country_code_to_flag_emoji(&country.code) + " ",
			None => String::new(),
		};

		let diff_string_if_not_self = if is_self {
			String::from("       ")
		} else {
			format!(
				"({:+.02})",
				entry.rating.get(skillset) - self_entry.rating.get(skillset)
			)
		};

		output += &format!(
			"{prefix}#{rank} | {rating:.02} {diff} | {flag}{user}\n",
			prefix = if is_self { "> " } else { "  " },
			rank = entry.rank,
			rating = entry.rating.get(skillset),
			diff = diff_string_if_not_self,
			flag = flag_emoji,
			user = entry.username,
		);
	}
	output += "```";

	poise::send_reply(ctx, |f| {
		f.embed(|f| f.color(crate::ETTERNA_COLOR).description(output))
	})
	.await?;

	Ok(())
}
