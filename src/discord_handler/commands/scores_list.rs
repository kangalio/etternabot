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
	limit: u32,
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

	// Download top scores, either via V1 or web API
	let top_scores: Result<Vec<Score>, etternaonline_api::Error> = match skillset {
		SkillOrAcc::Skillset(skillset) => {
			let scores = ctx
				.data()
				.v1
				.user_top_scores(&username, skillset, limit)
				.await;
			scores.map(|scores| scores.into_iter().map(Score::V1).collect::<Vec<_>>())
		}
		SkillOrAcc::Accuracy => {
			let scores = ctx
				.data()
				.web
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

	let country_code = ctx.data().v1.user_data(&username).await?.country_code;

	let mut scorekeys = Vec::new();

	let mut response = String::from("```");
	let mut i: u32 = 1;
	for entry in &top_scores {
		let (song_name, rate, ssr_overall, wifescore, scorekey) = match entry {
			Score::Web(s) => {
				let more = match &s.validity_dependant {
					Some(x) => x,
					None => continue,
				};
				(
					&s.song_name,
					s.rate,
					more.ssr_overall_nerfed,
					s.wifescore,
					more.scorekey.clone(),
				)
			}
			Score::V1(s) => (
				&s.song_name,
				s.rate,
				s.ssr_overall,
				s.wifescore,
				s.scorekey.clone(),
			),
		};

		scorekeys.push(scorekey);

		response += &format!(
			"{}. {}: {}\n  ▸ Score: {:.2} Wife: {:.2}%\n",
			i,
			song_name,
			rate,
			ssr_overall,
			wifescore.as_percent(),
		);
		i += 1;
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

	ctx.data()
		.lock_data()
		.last_scores_list
		.insert(ctx.channel_id(), scorekeys);

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

	let latest_scores = ctx.data().v2().await?.user_latest_scores(&username).await?;

	let country_code = ctx.data().v1.user_data(&username).await?.country_code;

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

	let scorekeys = latest_scores.into_iter().map(|s| s.scorekey).collect();
	ctx.data()
		.lock_data()
		.last_scores_list
		.insert(ctx.channel_id(), scorekeys);

	Ok(())
}

/// Show details about a specific score from a previous score list
#[poise::command(track_edits, slash_command)]
pub async fn details(
	ctx: Context<'_>,
	#[description = "Number of the score"] position: usize,
	#[description = "Specific judge to use for statistics"] judge: Option<
		poise::Wrapper<super::Judge>,
	>,
) -> Result<(), Error> {
	let scorekey = {
		let data = ctx.data().lock_data();

		let scores = data
			.last_scores_list
			.get(&ctx.channel_id())
			.ok_or("No score list has been posted in this channel")?;

		position
			.checked_sub(1)
			.and_then(|i| scores.get(i))
			.ok_or_else(|| format!("Enter a number between 1-{}", scores.len()))?
			.clone()
	};

	super::send_score_card(
		ctx.data(),
		ctx.discord(),
		ctx.channel_id(),
		super::ScoreCard {
			alternative_judge: judge.map(|x| x.0 .0),
			scorekey: &scorekey,
			show_ssrs_and_judgements_and_modifiers: true,
			user_id: None,
		},
	)
	.await?;

	Ok(())
}
