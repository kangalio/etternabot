use super::{Context, State};
use crate::{serenity, Error};

pub const CMD_TOP_HELP: &str =
	"Call this command with `+top[NN] [USERNAME] [SKILLSET]` (both params optional)";

pub fn top_scores(
	state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
	args: &str,
	mut limit: u32,
) -> Result<(), Error> {
	if !(1..=30).contains(&limit) {
		msg.channel_id
			.say(&ctx.http, "Only limits up to 30 are supported")?;
		return Ok(());
	}

	let args: Vec<&str> = args.split_whitespace().collect();

	let skillset;
	let eo_username;
	match *args.as_slice() {
		[] => {
			skillset = None;
			eo_username = state.get_eo_username(ctx, msg)?;
		}
		[skillset_or_username] => match etterna::Skillset7::from_user_input(skillset_or_username) {
			Some(parsed_skillset) => {
				skillset = Some(parsed_skillset);
				eo_username = state.get_eo_username(ctx, msg)?;
			}
			None => {
				skillset = None;
				eo_username = skillset_or_username.to_owned();
			}
		},
		[skillset_str, username] => {
			skillset = match etterna::Skillset7::from_user_input(skillset_str) {
				Some(parsed_skillset) => Some(parsed_skillset),
				None => {
					msg.channel_id.say(
						&ctx.http,
						format!("Unrecognized skillset \"{}\"", skillset_str),
					)?;
					return Ok(());
				}
			};
			eo_username = username.to_owned();
		}
		_ => {
			msg.channel_id.say(&ctx.http, CMD_TOP_HELP)?;
			return Ok(());
		}
	}

	// Download top scores
	let top_scores = match skillset {
		None => state.v2()?.user_top_10_scores(&eo_username),
		Some(skillset) => state
			.v2()?
			.user_top_skillset_scores(&eo_username, skillset, limit),
	};
	if let Err(etternaonline_api::Error::UserNotFound) = top_scores {
		msg.channel_id.say(
			&ctx.http,
			format!("No such user or skillset \"{}\"", eo_username),
		)?;
		return Ok(());
	}
	let top_scores = top_scores?;

	let country_code = state.v2()?.user_details(&eo_username)?.country_code;

	let mut response = String::from("```");
	for (i, entry) in top_scores.iter().enumerate() {
		response += &format!(
			"{}. {}: {}\n  ▸ Score: {:.2} Wife: {:.2}%\n",
			i + 1,
			&entry.song_name,
			entry.rate,
			entry.ssr_overall,
			entry.wifescore.as_percent(),
		);
	}

	if limit != 10 && skillset == None {
		limit = 10;
		response += "(due to a bug in the EO v2 API, only 10 entries can be shown in Overall mode)";
	}

	response += "```";

	let title = match skillset {
		None => format!("{}'s Top {}", eo_username, limit),
		Some(skillset) => format!("{}'s Top {} {}", eo_username, limit, skillset),
	};

	msg.channel_id.send_message(&ctx.http, |m| {
		m.embed(|e| {
			e.color(crate::ETTERNA_COLOR)
				.description(&response)
				.author(|a| {
					a.name(title)
						.url(format!(
							"https://etternaonline.com/user/profile/{}",
							eo_username
						))
						.icon_url(format!(
							"https://etternaonline.com/img/flags/{}.png",
							country_code
						))
				})
		})
	})?;

	Ok(())
}

pub fn latest_scores(ctx: Context<'_>, args: &str) -> Result<(), Error> {
	let eo_username = poise::parse_args!(args => (Option<String>))?;
	let eo_username = match eo_username {
		Some(x) => x,
		None => ctx.data.get_eo_username(&ctx.discord, &ctx.msg)?,
	};

	let latest_scores = ctx.data.v2()?.user_latest_scores(&eo_username)?;

	let country_code = ctx.data.v2()?.user_details(&eo_username)?.country_code;

	let mut response = String::from("```");
	for (i, entry) in latest_scores.iter().enumerate() {
		response += &format!(
			"{}. {}: {}\n  ▸ Score: {:.2} Wife: {:.2}%\n",
			i + 1,
			&entry.song_name,
			entry.rate,
			entry.ssr_overall,
			entry.wifescore.as_percent(),
		);
	}
	response += "```";

	let title = format!("{}'s Last 10 Scores", eo_username);

	poise::send_reply(ctx, |m| {
		m.embed(|e| {
			e.color(crate::ETTERNA_COLOR)
				.description(&response)
				.author(|a| {
					a.name(title)
						.url(format!(
							"https://etternaonline.com/user/profile/{}",
							eo_username
						))
						.icon_url(format!(
							"https://etternaonline.com/img/flags/{}.png",
							country_code
						))
				})
		})
	})?;

	Ok(())
}
