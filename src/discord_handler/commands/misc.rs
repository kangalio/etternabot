use super::State;
use crate::{serenity, Error};

const CMD_USERSET_HELP: &str = "Call this command with `+userset YOUR_EO_USERNAME`";
const CMD_RIVALSET_HELP: &str = "Call this command with `+rivalset YOUR_EO_USERNAME`";
const CMD_SCROLLSET_HELP: &str = "Call this command with `+scrollset [down/up]`";
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
		.ok_or(Error::UserNotInRegistry)?;

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

pub fn profile(
	state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
	text: &str,
) -> Result<(), Error> {
	let eo_username = if text.is_empty() {
		state.get_eo_username(ctx, msg)?
	} else {
		text.to_owned()
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
					"{: >10}:   {: >5.2} (+{: >4.2})  #{: <4}\n",
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

	// TODO: could create new entry if doesn't already exist to store ratings
	if let Some(previous_ratings) = previous_ratings {
		*previous_ratings = Some(details.rating.clone());
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
					html2md::parse_html(&details.about_me),
					false,
				);
			}

			embed
		})
	})?;

	Ok(())
}
