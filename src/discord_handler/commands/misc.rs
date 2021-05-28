//! Miscellaneous "fun-fact" commands

use super::{Context, PrefixContext};
use crate::Error;

#[poise::command(track_edits)]
pub async fn ping(ctx: PrefixContext<'_>, #[rest] args: String) -> Result<(), Error> {
	let mut response = String::from("Pong");
	for _ in 0..args.matches("ping").count() {
		response += " pong";
	}
	response += "!";
	poise::say_prefix_reply(ctx, response).await?;

	Ok(())
}

#[poise::command(track_edits)]
pub async fn servers(ctx: PrefixContext<'_>) -> Result<(), Error> {
	let current_user = ctx.discord.http.get_current_user().await?;
	let guilds = current_user.guilds(ctx.discord).await?;

	let mut response = format!("I am currently in {} servers!\n", guilds.len());
	for guild in guilds {
		response += &format!("- {}\n", guild.name);
	}

	poise::say_prefix_reply(ctx, response).await?;

	Ok(())
}

#[poise::command(track_edits)]
pub async fn uptime(ctx: PrefixContext<'_>) -> Result<(), Error> {
	let uptime = std::time::Instant::now() - ctx.data.bot_start_time;

	let div_mod = |a, b| (a / b, a % b);

	let millis = uptime.as_millis();
	let (seconds, millis) = div_mod(millis, 1000);
	let (minutes, seconds) = div_mod(seconds, 60);
	let (hours, minutes) = div_mod(minutes, 60);
	let (days, hours) = div_mod(hours, 24);

	poise::say_prefix_reply(
		ctx,
		format!(
			"Duration since last restart: {}:{:02}:{:02}:{:02}.{:03}",
			days, hours, minutes, seconds, millis
		),
	)
	.await?;

	Ok(())
}

/// Lookup a saved user by their Discord username
///
/// Call this command with `+lookup DISCORDUSERNAME`
#[poise::command(track_edits, slash_command)]
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
			"Discord username: {}\nEO username: {}\nhttps://etternaonline.com/user/{}",
			user.discord_username, user.eo_username, user.eo_username,
		),
	)
	.await?;

	Ok(())
}

/// Print one of various random quotes, phrases and memes from various rhythm gaming communities
#[poise::command(track_edits, slash_command)]
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

/// Register slash commands in this guild or globally
///
/// Run with no arguments to register in guild, run with argument "global" to register globally.
#[poise::command(hide_in_help)]
pub async fn register(ctx: PrefixContext<'_>, #[flag] global: bool) -> Result<(), Error> {
	// REMEMBER: hardcoded id is bad
	if ctx.msg.author.id.0 != 472029906943868929 {
		return Err("You're not kangalioo".into());
	}

	let guild_id = ctx.msg.guild_id.ok_or("Must be called in guild")?;

	let commands = &ctx.framework.options().slash_options.commands;
	poise::say_prefix_reply(ctx, format!("Registering {} commands...", commands.len())).await?;
	for cmd in commands {
		if global {
			cmd.create_global(&ctx.discord.http).await?;
		} else {
			cmd.create_in_guild(&ctx.discord.http, guild_id).await?;
		}
	}

	poise::say_prefix_reply(ctx, "Done!".to_owned()).await?;

	Ok(())
}
