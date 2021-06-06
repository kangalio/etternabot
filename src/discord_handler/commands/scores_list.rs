use super::Context;
use crate::Error;

#[derive(PartialEq)]
pub enum SkillOrAcc {
	Skillset(etterna::Skillset8),
	Accuracy,
}

impl std::str::FromStr for SkillOrAcc {
	type Err = etterna::UnrecognizedSkillset;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		if s.eq_ignore_ascii_case("acc") || s.eq_ignore_ascii_case("accuracy") {
			Ok(Self::Accuracy)
		} else {
			Ok(Self::Skillset(s.parse::<etterna::Skillset8>()?))
		}
	}
}

/// Show a user's top 10 scores with the highest rating
///
/// Call this command with `+top10 [USERNAME] [SKILLSET]` (username and skillset optional)
#[poise::command(track_edits, slash_command)]
pub async fn top10(
	ctx: Context<'_>,
	#[description = "Falls back to your username"]
	#[lazy]
	username: Option<String>,
	#[description = "Specific skillset to focus on"] skillset: Option<poise::Wrapper<SkillOrAcc>>,
) -> Result<(), Error> {
	topscores(ctx, 10, skillset.map(|x| x.0), username).await
}

/// Show a user's top scores with the highest rating
///
/// Call this command with `+top [NN] [USERNAME] [SKILLSET]` (username and skillset optional)
#[poise::command(track_edits, slash_command)]
pub async fn top(
	ctx: Context<'_>,
	#[description = "Number of scores to show"] limit: u32,
	#[description = "Falls back to your username"]
	#[lazy]
	username: Option<String>,
	#[description = "Specific skillset to focus on"] skillset: Option<poise::Wrapper<SkillOrAcc>>,
) -> Result<(), Error> {
	topscores(ctx, limit, skillset.map(|x| x.0), username).await
}

async fn topscores(
	ctx: Context<'_>,
	mut limit: u32,
	skillset: Option<SkillOrAcc>,
	username: Option<String>,
) -> Result<(), Error> {
	let username = match username {
		Some(x) => x,
		None => ctx.data().get_eo_username(ctx.author()).await?,
	};

	if !(1..=30).contains(&limit) {
		poise::say_reply(ctx, "Only limits up to 30 are supported".into()).await?;
		return Ok(());
	}

	let skillset = skillset.unwrap_or(SkillOrAcc::Skillset(etterna::Skillset8::Overall));

	enum Score {
		V1(etternaonline_api::v1::TopScore),
		Web(etternaonline_api::web::UserScore),
	}

	// Download top scores, either via V2 or web API
	let top_scores: Result<Vec<Score>, etternaonline_api::Error> = match skillset {
		SkillOrAcc::Skillset(skillset) => {
			let scores = ctx
				.data()
				.v2()
				.await?
				.user_top_scores(&username, skillset, limit)
				.await;
			scores.map(|scores| scores.into_iter().map(Score::V1).collect::<Vec<_>>())
		}
		SkillOrAcc::Accuracy => {
			let scores = ctx
				.data()
				.web_session
				.user_scores(
					ctx.data().get_eo_user_id(&username).await?,
					0..limit,
					None,
					etternaonline_api::web::UserScoresSortBy::Wifescore,
					etternaonline_api::web::SortDirection::Descending,
					false,
				)
				.await;
			scores.map(|s| s.scores.into_iter().map(Score::Web).collect::<Vec<_>>())
		}
	};
	if let Err(etternaonline_api::Error::UserNotFound { name: _ }) = top_scores {
		poise::say_reply(ctx, format!("No such user or skillset \"{}\"", username)).await?;
		return Ok(());
	}
	let top_scores = top_scores?;

	let country_code = ctx
		.data()
		.v2()
		.await?
		.user_data(&username)
		.await?
		.country_code;

	let mut response = String::from("```");
	for (i, entry) in top_scores.iter().enumerate() {
		let (song_name, rate, ssr_overall, wifescore) = match entry {
			Score::Web(s) => {
				let more = match &s.validity_dependant {
					Some(x) => x,
					None => continue,
				};
				(&s.song_name, s.rate, more.ssr_overall_nerfed, s.wifescore)
			}
			Score::V1(s) => (&s.song_name, s.rate, s.ssr_overall, s.wifescore),
		};
		response += &format!(
			"{}. {}: {}\n  ▸ Score: {:.2} Wife: {:.2}%\n",
			i + 1,
			song_name,
			rate,
			ssr_overall,
			wifescore.as_percent(),
		);
	}

	if limit != 10 && skillset == SkillOrAcc::Skillset(etterna::Skillset8::Overall) {
		limit = 10;
		response += "(due to a bug in the EO v2 API, only 10 entries can be shown in Overall mode)";
	}

	response += "```";

	let title = match skillset {
		SkillOrAcc::Skillset(etterna::Skillset8::Overall) => {
			format!("{}'s Top {}", username, limit)
		}
		SkillOrAcc::Skillset(skillset) => format!("{}'s Top {} {}", username, limit, skillset),
		SkillOrAcc::Accuracy => format!("{}'s Top {} Accuracy", username, limit),
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
							country_code.as_deref().unwrap_or("")
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
		None => ctx.data().get_eo_username(ctx.author()).await?,
	};

	let latest_scores = ctx.data().v2().await?.user_latest_10_scores(&username).await?;

	let country_code = ctx
		.data()
		.v2()
		.await?
		.user_data(&username)
		.await?
		.country_code;

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
							country_code.as_deref().unwrap_or("")
						))
				})
		})
	})
	.await?;

	Ok(())
}
