use super::Context;
use crate::Error;

/// Call this command with `+top [NN] [USERNAME] [SKILLSET]` (username and skillset optional)
#[poise::command(track_edits, slash_command)]
pub async fn top_scores(
	ctx: Context<'_>,
	#[description = "Number of scores to show"] mut limit: u32,
	#[description = "Specific skillset to focus on"] skillset: Option<
		poise::Wrapper<etterna::Skillset7>,
	>,
	#[description = "Falls back to your username"] username: Option<String>,
) -> Result<(), Error> {
	let username = match username {
		Some(x) => x,
		None => ctx.data().get_eo_username(ctx.author())?,
	};

	if !(1..=30).contains(&limit) {
		poise::say_reply(ctx, "Only limits up to 30 are supported".into()).await?;
		return Ok(());
	}

	// Download top scores
	let top_scores = match skillset {
		None => ctx.data().v2()?.user_top_10_scores(&username),
		Some(skillset) => ctx
			.data()
			.v2()?
			.user_top_skillset_scores(&username, skillset.0, limit),
	};
	if let Err(etternaonline_api::Error::UserNotFound) = top_scores {
		poise::say_reply(ctx, format!("No such user or skillset \"{}\"", username)).await?;
		return Ok(());
	}
	let top_scores = top_scores?;

	let country_code = ctx.data().v2()?.user_details(&username)?.country_code;

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
		None => format!("{}'s Top {}", username, limit),
		Some(skillset) => format!("{}'s Top {} {}", username, limit, skillset.0),
	};

	poise::send_reply(ctx, |m| {
		m.embed(|e| {
			e.color(crate::ETTERNA_COLOR)
				.description(&response)
				.author(|a| {
					a.name(title)
						.url(format!(
							"https://etternaonline.com/user/profile/{}",
							username
						))
						.icon_url(format!(
							"https://etternaonline.com/img/flags/{}.png",
							country_code
						))
				})
		})
	})
	.await?;

	Ok(())
}

/// Show a list of recent scores
#[poise::command(aliases("ls"), track_edits, slash_command)]
pub async fn lastsession(
	ctx: Context<'_>,
	#[description = "Falls back to your username"] username: Option<String>,
) -> Result<(), Error> {
	let username = match username {
		Some(x) => x,
		None => ctx.data().get_eo_username(ctx.author())?,
	};

	let latest_scores = ctx.data().v2()?.user_latest_scores(&username)?;

	let country_code = ctx.data().v2()?.user_details(&username)?.country_code;

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

	let title = format!("{}'s Last 10 Scores", username);

	poise::send_reply(ctx, |m| {
		m.embed(|e| {
			e.color(crate::ETTERNA_COLOR)
				.description(&response)
				.author(|a| {
					a.name(title)
						.url(format!(
							"https://etternaonline.com/user/profile/{}",
							username
						))
						.icon_url(format!(
							"https://etternaonline.com/img/flags/{}.png",
							country_code
						))
				})
		})
	})
	.await?;

	Ok(())
}
