//! Miscellaneous "fun-fact" commands

use crate::{Context, Error};

#[poise::command(prefix_command, track_edits)]
pub async fn ping(ctx: Context<'_>, #[rest] args: Option<String>) -> Result<(), Error> {
	let args = args.as_deref().unwrap_or("");

	let mut response = String::from("Pong");
	for _ in 0..args.matches("ping").count() {
		response += " pong";
	}
	response += "!";
	poise::say_reply(ctx, response).await?;

	Ok(())
}

/// List servers of which the bot is a member of
#[poise::command(prefix_command, slash_command, track_edits, hide_in_help)]
pub async fn servers(ctx: Context<'_>) -> Result<(), Error> {
	poise::samples::servers(ctx).await?;

	Ok(())
}

#[poise::command(prefix_command, track_edits)]
pub async fn uptime(ctx: Context<'_>) -> Result<(), Error> {
	let uptime = std::time::Instant::now() - ctx.data().bot_start_time;

	let div_mod = |a, b| (a / b, a % b);

	let seconds = uptime.as_secs();
	let (minutes, seconds) = div_mod(seconds, 60);
	let (hours, minutes) = div_mod(minutes, 60);
	let (days, hours) = div_mod(hours, 24);

	poise::say_reply(
		ctx,
		format!(
			"Duration since last restart: {}d {}h {}m {}s",
			days, hours, minutes, seconds
		),
	)
	.await?;

	Ok(())
}

/// Lookup a saved user by their Discord username
///
/// Call this command with `+lookup DISCORDUSERNAME`
#[poise::command(prefix_command, track_edits, slash_command)]
pub async fn lookup(
	ctx: Context<'_>,
	#[description = "Discord username"] discord_username: String,
) -> Result<(), Error> {
	let user = ctx
		.data()
		.lock_data()
		.user_registry
		.iter()
		.find(|user| {
			user.discord_username
				.eq_ignore_ascii_case(&discord_username)
		})
		.ok_or(crate::MISSING_REGISTRY_ENTRY_ERROR_MESSAGE)?
		.clone();

	poise::say_reply(
		ctx,
		format!(
			"Discord: **{}** (ID {})\nEtternaOnline: **{}** (ID {})\nhttps://etternaonline.com/user/{}",
			user.discord_username, user.discord_id, user.eo_username, user.eo_id, user.eo_username,
		),
	)
	.await?;

	Ok(())
}

/// Print one of various random quotes, phrases and memes from various rhythm gaming communities
#[poise::command(prefix_command, track_edits, slash_command)]
pub async fn quote(ctx: Context<'_>) -> Result<(), Error> {
	use rand::Rng as _;

	let quote_index = rand::thread_rng().gen_range(0, ctx.data().config.quotes.len());
	// UNWRAP: index is below quotes len because we instructed the rand crate to do so
	let quote = ctx.data().config.quotes.get(quote_index).unwrap();
	let string = match &quote.source {
		Some(source) => format!("> {}\n~ {}", quote.quote, source),
		None => format!("> {}", quote.quote),
	};
	poise::say_reply(ctx, string).await?;

	Ok(())
}

/// Registers slash commands in this guild or globally
///
/// Run with no arguments to register in guild, run with argument "global" to register globally.
#[poise::command(prefix_command, hide_in_help)]
pub async fn register(ctx: Context<'_>, #[flag] global: bool) -> Result<(), Error> {
	let _typing = ctx.defer_or_broadcast().await;

	poise::samples::register_application_commands(ctx, global).await?;

	Ok(())
}
