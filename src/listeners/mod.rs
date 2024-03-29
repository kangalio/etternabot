//! All code that listens for certain discord events and reacts somehow

use crate::{serenity, Error, PrefixContext};

fn contains_link(string: &str) -> bool {
	static LINK_REGEX: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
		regex::Regex::new(
			r"http[s]?://(?:[a-zA-Z]|[0-9]|[$-_@.&+]|[!*\(\),]|(?:%[0-9a-fA-F][0-9a-fA-F]))+",
		)
		.unwrap()
	});

	LINK_REGEX.find_iter(string).count() >= 1
}

// struct Scorekey(String);
// impl Scorekey {
// 	fn new(s: String) -> Option<Self> {
// 		Some(Self(s))
// 	}
// }

fn extract_score_links_from_string(
	string: &str,
) -> impl Iterator<Item = (etterna::Scorekey, u32)> + Send + '_ {
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
				log::warn!(
					"Error while parsing '{}' (\\d+) as u32: {}",
					user_id_group,
					e
				)
			})
			.ok()?;

		Some((scorekey, user_id))
	})
}

async fn show_score_links_inside_message(ctx: PrefixContext<'_>) {
	let alternative_judge = crate::extract_judge_from_string(&ctx.msg.content);
	let mut score_links = extract_score_links_from_string(&ctx.msg.content);
	if let Some((scorekey, user_id)) = score_links.next() {
		log::info!(
			"Trying to show score card for scorekey {} user id {}",
			scorekey,
			user_id
		);
		if let Err(e) = crate::send_score_card(
			poise::Context::Prefix(ctx),
			crate::ScoreCard {
				scorekey: &scorekey,
				user_id: None,
				username: None,
				show_ssrs_and_judgements_and_modifiers: true,
				alternative_judge,
				draw_mean_instead_of_wifescore: ctx.msg.content.contains("mean"),
			},
		)
		.await
		{
			log::warn!("Error while showing score card for {}: {}", scorekey, e);
		}
	}

	let num_score_links = score_links.count() + 1;
	if num_score_links > 1 {
		log::info!("Refusing to show all {} score links", num_score_links);
	}
}

pub async fn listen_message(
	ctx: PrefixContext<'_>,
	has_manage_messages_permission: bool,
	user_is_allowed_bot_interaction: bool,
) -> Result<(), Error> {
	if ctx.msg.channel_id == ctx.data.config.work_in_progress_channel
		&& !has_manage_messages_permission
	{
		if !contains_link(&ctx.msg.content) && ctx.msg.attachments.is_empty() {
			ctx.msg.delete(ctx.serenity_context).await?;
			let notice_msg = ctx
				.msg
				.channel_id
				.say(
					ctx.serenity_context,
					format!(
					"Only links and attachments are allowed in this channel. For discussions use <#{}>",
					ctx.data.config.work_in_progress_discussion_channel),
				)
				.await?;
			tokio::time::sleep(std::time::Duration::from_millis(5000)).await;
			notice_msg.delete(ctx.serenity_context).await?;
			return Ok(());
		}
	}

	if ctx.msg.channel_id == ctx.data.config.pack_releases_channel
		&& !has_manage_messages_permission
	{
		if !contains_link(&ctx.msg.content) && ctx.msg.attachments.is_empty() {
			ctx.msg.delete(ctx.serenity_context).await?;
			let notice_msg = ctx
				.msg
				.channel_id
				.say(
					ctx.serenity_context,
					"Only links and attachments are allowed in this channel.",
				)
				.await?;
			tokio::time::sleep(std::time::Duration::from_millis(5000)).await;
			notice_msg.delete(ctx.serenity_context).await?;
			return Ok(());
		}
	}

	if user_is_allowed_bot_interaction {
		show_score_links_inside_message(ctx).await;
	}

	Ok(())
}

pub async fn check_member_update_for_max_300(
	state: &crate::State,
	ctx: &serenity::Context,
	old: &serenity::Member,
	new: &serenity::Member,
) -> Result<(), Error> {
	let guild = new.guild_id.to_partial_guild(&ctx.http).await?;

	let get_guild_role = |guild_id| {
		if let Some(guild) = guild.roles.get(guild_id) {
			Some(guild.name.as_str())
		} else {
			log::warn!(
				"Couldn't find role {:?} in guild roles ({:?})... weird",
				guild_id,
				guild.roles
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
			.to_channel(ctx)
			.await?
			// UNWRAP: we verified in State::load()
			.guild()
			.unwrap()
			.say(
				&ctx.http,
				format!("Congrats on the promotion, <@{}>!", old.user.id),
			)
			.await?;
	}

	Ok(())
}

pub async fn guild_member_update(
	state: &crate::State,
	ctx: &serenity::Context,
	old: Option<&serenity::Member>,
	new: &serenity::Member,
) -> Result<(), Error> {
	if let Some(user_entry) = state
		.lock_data()
		.user_registry
		.iter_mut()
		.find(|user| user.discord_id == new.user.id)
	{
		user_entry.discord_username = new.user.name.clone();
	} else {
		// TODO: integrate into registry?
	}

	if let Some(old) = old {
		check_member_update_for_max_300(state, ctx, old, new).await?;
	}

	Ok(())
}
