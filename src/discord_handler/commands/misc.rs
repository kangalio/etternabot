//! Miscellaneous "fun-fact" commands

use super::Context;
use crate::Error;

pub fn ping(ctx: Context<'_>, args: &str) -> Result<(), Error> {
	let mut response = String::from("Pong");
	for _ in 0..args.matches("ping").count() {
		response += " pong";
	}
	response += "!";
	poise::say_reply(ctx, response)?;

	Ok(())
}

pub fn servers(ctx: Context<'_>, _args: &str) -> Result<(), Error> {
	let guilds = ctx.discord.http.get_current_user()?.guilds(ctx.discord)?;

	let mut response = format!("I am currently in {} servers!\n", guilds.len());
	for guild in guilds {
		response += &format!("- {}\n", guild.name);
	}

	poise::say_reply(ctx, response)?;

	Ok(())
}

pub fn uptime(ctx: Context<'_>, _args: &str) -> Result<(), Error> {
	let uptime = std::time::Instant::now() - ctx.data.bot_start_time;

	let div_mod = |a, b| (a / b, a % b);

	let millis = uptime.as_millis();
	let (seconds, millis) = div_mod(millis, 1000);
	let (minutes, seconds) = div_mod(seconds, 60);
	let (hours, minutes) = div_mod(minutes, 60);
	let (days, hours) = div_mod(hours, 24);

	poise::say_reply(
		ctx,
		format!(
			"Duration since last restart: {}:{:02}:{:02}:{:02}.{:03}",
			days, hours, minutes, seconds, millis
		),
	)?;

	Ok(())
}

pub fn lookup(ctx: Context<'_>, args: &str) -> Result<(), Error> {
	let discord_username = poise::parse_args!(args => (String))?;

	let data = ctx.data.lock_data();
	let user = data
		.user_registry
		.iter()
		.find(|user| {
			user.discord_username
				.eq_ignore_ascii_case(&discord_username)
		})
		.ok_or(crate::MISSING_REGISTRY_ENTRY_ERROR_MESSAGE)?;

	poise::say_reply(
		ctx,
		format!(
			"Discord username: {}\nEO username: {}\nhttps://etternaonline.com/user/{}",
			user.discord_username, user.eo_username, user.eo_username,
		),
	)?;

	Ok(())
}

pub fn quote(ctx: Context<'_>, _args: &str) -> Result<(), Error> {
	use rand::Rng as _;

	let quote_index = rand::thread_rng().gen_range(0, ctx.data.config.quotes.len());
	// UNWRAP: index is below quotes len because we instructed the rand crate to do so
	let quote = ctx.data.config.quotes.get(quote_index).unwrap();
	let string = match &quote.source {
		Some(source) => format!("> {}\n~ {}", quote.quote, source),
		None => format!("> {}", quote.quote),
	};
	poise::say_reply(ctx, string)?;

	Ok(())
}
