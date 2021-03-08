//! Miscellaneous "fun-fact" commands

use super::State;
use crate::{serenity, Error};

const CMD_LOOKUP_HELP: &str = "Call this command with `+lookup DISCORDUSERNAME`";

pub fn ping(
	_state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
	args: &str,
) -> Result<(), Error> {
	let mut response = String::from("Pong");
	for _ in 0..args.matches("ping").count() {
		response += " pong";
	}
	response += "!";
	msg.channel_id.say(&ctx.http, &response)?;

	Ok(())
}

pub fn servers(
	_state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
	_args: &str,
) -> Result<(), Error> {
	let guilds = ctx.http.get_current_user()?.guilds(&ctx.http)?;

	let mut response = format!("I am currently in {} servers!\n", guilds.len());
	for guild in guilds {
		response += &format!("- {}\n", guild.name);
	}

	msg.channel_id.say(&ctx.http, response)?;

	Ok(())
}

pub fn uptime(
	state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
	_args: &str,
) -> Result<(), Error> {
	let uptime = std::time::Instant::now() - state.bot_start_time;

	let div_mod = |a, b| (a / b, a % b);

	let millis = uptime.as_millis();
	let (seconds, millis) = div_mod(millis, 1000);
	let (minutes, seconds) = div_mod(seconds, 60);
	let (hours, minutes) = div_mod(minutes, 60);
	let (days, hours) = div_mod(hours, 24);

	msg.channel_id.say(
		&ctx.http,
		format!(
			"Duration since last restart: {}:{:02}:{:02}:{:02}.{:03}",
			days, hours, minutes, seconds, millis
		),
	)?;

	Ok(())
}

pub fn lookup(
	state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
	args: &str,
) -> Result<(), Error> {
	if args.is_empty() {
		msg.channel_id.say(&ctx.http, CMD_LOOKUP_HELP)?;
		return Ok(());
	}

	let data = state.lock_data();
	let user = data
		.user_registry
		.iter()
		.find(|user| user.discord_username.eq_ignore_ascii_case(args))
		.ok_or(crate::MISSING_REGISTRY_ENTRY_ERROR_MESSAGE)?;

	msg.channel_id.say(
		&ctx.http,
		format!(
			"Discord username: {}\nEO username: {}\nhttps://etternaonline.com/user/{}",
			user.discord_username, user.eo_username, user.eo_username,
		),
	)?;

	Ok(())
}

pub fn quote(
	state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
	_args: &str,
) -> Result<(), Error> {
	use rand::Rng as _;

	let quote_index = rand::thread_rng().gen_range(0, state.config.quotes.len());
	// UNWRAP: index is below quotes len because we instructed the rand crate to do so
	let quote = state.config.quotes.get(quote_index).unwrap();
	let string = match &quote.source {
		Some(source) => format!("> {}\n~ {}", quote.quote, source),
		None => format!("> {}", quote.quote),
	};
	msg.channel_id.say(&ctx.http, &string)?;

	Ok(())
}
