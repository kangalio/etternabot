//! All code that listens for certain discord events and reacts somehow

mod score_ocr;
pub use score_ocr::{reaction_add, OcrError, OcrScoreCardManager};

use super::*;
use crate::{serenity, Error};

fn contains_link(string: &str) -> bool {
	static LINK_REGEX: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
		regex::Regex::new(
			r"http[s]?://(?:[a-zA-Z]|[0-9]|[$-_@.&+]|[!*\(\),]|(?:%[0-9a-fA-F][0-9a-fA-F]))+",
		)
		.unwrap()
	});

	LINK_REGEX.find_iter(string).count() >= 1
}

fn extract_score_links_from_string(
	string: &str,
) -> impl Iterator<Item = (etterna::Scorekey, u32)> + '_ {
	static SCORE_LINK_REGEX: once_cell::sync::Lazy<regex::Regex> =
		once_cell::sync::Lazy::new(|| {
			regex::Regex::new(r"https://etternaonline.com/score/view/(S\w{40})(\d+)").unwrap()
		});

	SCORE_LINK_REGEX.captures_iter(string).filter_map(|groups| {
		let scorekey = etterna::Scorekey::new(groups.get(1).unwrap().as_str().to_owned())?;

		// UNWRAP: regex has this group
		let user_id_group = groups.get(2).unwrap().as_str();
		let user_id: u32 = user_id_group
			.parse()
			.map_err(|e| {
				println!(
					"Error while parsing '{}' (\\d+) as u32: {}",
					user_id_group, e
				)
			})
			.ok()?;

		Some((scorekey, user_id))
	})
}

fn show_score_links_inside_message(
	state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
) {
	let alternative_judge = super::extract_judge_from_string(&msg.content);
	for (scorekey, user_id) in extract_score_links_from_string(&msg.content) {
		println!(
			"Trying to show score card for scorekey {} user id {}",
			scorekey, user_id
		);
		if let Err(e) = super::send_score_card(
			state,
			&ctx,
			msg.channel_id,
			super::ScoreCard {
				scorekey: &scorekey,
				user_id: None,
				show_ssrs_and_judgements_and_modifiers: true,
				alternative_judge,
			},
		) {
			println!("Error while showing score card for {}: {}", scorekey, e);
		}
	}
}

pub fn listen_message(
	state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
	has_manage_messages_permission: bool,
	user_is_allowed_bot_interaction: bool,
) -> Result<(), Error> {
	score_ocr::check_potential_score_screenshot(state, ctx, msg)?;

	if msg.channel_id == state.config.work_in_progress_channel && !has_manage_messages_permission {
		if !contains_link(&msg.content) && msg.attachments.is_empty() {
			msg.delete(&ctx.http)?;
			let notice_msg = msg.channel_id.say(
				&ctx.http,
				format!(
					"Only links and attachments are allowed in this channel. For discussions use <#{}>",
					state.config.work_in_progress_discussion_channel),
			)?;
			std::thread::sleep(std::time::Duration::from_millis(5000));
			notice_msg.delete(&ctx.http)?;
			return Ok(());
		}
	}

	if msg.channel_id == state.config.pack_releases_channel && !has_manage_messages_permission {
		if !contains_link(&msg.content) && msg.attachments.is_empty() {
			msg.delete(&ctx.http)?;
			let notice_msg = msg.channel_id.say(
				&ctx.http,
				"Only links and attachments are allowed in this channel.",
			)?;
			std::thread::sleep(std::time::Duration::from_millis(5000));
			notice_msg.delete(&ctx.http)?;
			return Ok(());
		}
	}

	if user_is_allowed_bot_interaction {
		show_score_links_inside_message(state, ctx, msg);
	}

	Ok(())
}

pub fn check_member_update_for_max_300(
	state: &State,
	ctx: serenity::Context,
	old: serenity::Member,
	new: serenity::Member,
) -> Result<(), Error> {
	let guild = new.guild_id.to_partial_guild(&ctx.http)?;

	let get_guild_role = |guild_id| {
		if let Some(guild) = guild.roles.get(guild_id) {
			Some(guild.name.as_str())
		} else {
			println!(
				"Couldn't find role {:?} in guild roles ({:?})... weird",
				guild_id, guild.roles
			);
			None
		}
	};

	let has_max_300_now = new
		.roles
		.iter()
		.any(|r| get_guild_role(r) == Some("MAX 300"));
	let had_max_300_previously = old
		.roles
		.iter()
		.any(|r| get_guild_role(r) == Some("MAX 300"));

	if has_max_300_now && !had_max_300_previously {
		state
			.config
			.promotion_gratulations_channel
			.to_channel(&ctx)?
			// UNWRAP: we verified in State::load()
			.guild()
			.unwrap()
			.read()
			.say(
				&ctx.http,
				format!("Congrats on the promotion, <@{}>!", old.user_id()),
			)?;
	}

	Ok(())
}

pub fn guild_member_update(
	state: &State,
	ctx: serenity::Context,
	old: Option<serenity::Member>,
	new: serenity::Member,
) -> Result<(), Error> {
	if let Some(user_entry) = state
		.lock_data()
		.user_registry
		.iter_mut()
		.find(|user| user.discord_id == new.user.read().id.0)
	{
		user_entry.discord_username = new.user.read().name.clone();
	} else {
		// TODO: integrate into registry?
	}

	if let Some(old) = old {
		check_member_update_for_max_300(state, ctx, old, new)?;
	}

	Ok(())
}
