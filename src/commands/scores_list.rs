//! All commands that show a list of scores

use crate::{Context, Error, Warn as _};

#[derive(PartialEq, Eq)]
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
#[poise::command(prefix_command, track_edits, slash_command)]
pub async fn top10(
	ctx: Context<'_>,
	#[description = "Falls back to your username"]
	#[lazy]
	#[autocomplete = "crate::autocomplete_username"]
	username: Option<String>,
	#[description = "Specific skillset to focus on"] skillset: Option<SkillOrAcc>,
) -> Result<(), Error> {
	topscores(ctx, 10, skillset, username).await
}

/// Show a user's top scores with the highest rating
///
/// Call this command with `+top [NN] [USERNAME] [SKILLSET]` (username and skillset optional)
#[poise::command(prefix_command, track_edits, slash_command)]
pub async fn top(
	ctx: Context<'_>,
	#[description = "Number of scores to show"] limit: u32,
	#[description = "Falls back to your username"]
	#[lazy]
	#[autocomplete = "crate::autocomplete_username"]
	username: Option<String>,
	#[description = "Specific skillset to focus on"] skillset: Option<SkillOrAcc>,
) -> Result<(), Error> {
	topscores(ctx, limit, skillset, username).await
}

struct ScoreEntry {
	song_name: String,
	rate: etterna::Rate,
	ssr_overall: f32,
	wifescore: etterna::Wifescore,
	scorekey: etterna::Scorekey,
}

async fn respond_score_list(
	ctx: Context<'_>,
	username: &str,
	title: &str,
	scorekeys: Vec<ScoreEntry>,
) -> Result<(), Error> {
	let mut response = String::from("```c\n");
	for (i, score) in scorekeys.iter().enumerate() {
		response += &format!(
			"{}. {}\n   {:.2}  {}  {:.2}%\n",
			i + 1,
			&score.song_name,
			score.ssr_overall,
			score.rate,
			score.wifescore.as_percent(),
		);
	}
	response += "```";

	let country_code = ctx
		.data()
		.v1
		.user_data(&username)
		.await
		.warn()
		.map_or(None, |u| u.country_code);

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

	let scorekeys = scorekeys.into_iter().map(|s| s.scorekey).collect();
	ctx.data().lock_data().last_scores_list.insert(
		ctx.channel_id(),
		crate::config::ScoresList {
			scorekeys,
			username: username.to_owned(),
		},
	);

	Ok(())
}

async fn topscores(
	ctx: Context<'_>,
	limit: u32,
	skillset: Option<SkillOrAcc>,
	username: Option<String>,
) -> Result<(), Error> {
	let _typing = ctx.defer_or_broadcast().await;

	let username = match username {
		Some(x) => x,
		None => ctx.data().get_eo_username(ctx.author()).await?,
	};

	if !(1..=30).contains(&limit) {
		poise::say_reply(ctx, "Only limits up to 30 are supported").await?;
		return Ok(());
	}

	let skillset = skillset.unwrap_or(SkillOrAcc::Skillset(etterna::Skillset8::Overall));

	// Download top scores, either via V1 or web API
	let scores = match skillset {
		SkillOrAcc::Skillset(skillset) => ctx
			.data()
			.v1
			.user_top_scores(&username, skillset, limit)
			.await
			.map_err(crate::no_such_user_or_skillset)?
			.into_iter()
			.map(|s| ScoreEntry {
				rate: s.rate,
				scorekey: s.scorekey,
				song_name: s.song_name,
				ssr_overall: s.ssr_overall,
				wifescore: s.wifescore,
			})
			.collect::<Vec<_>>(),
		SkillOrAcc::Accuracy => ctx
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
			.await
			.map_err(crate::no_such_user_or_skillset)?
			.scores
			.into_iter()
			.filter_map(|s| {
				let validity_dependant = s.validity_dependant?;
				Some(ScoreEntry {
					rate: s.rate,
					scorekey: validity_dependant.scorekey,
					song_name: s.song_name,
					ssr_overall: validity_dependant.ssr_overall_nerfed,
					wifescore: s.wifescore,
				})
			})
			.collect::<Vec<_>>(),
	};

	let title = match skillset {
		SkillOrAcc::Skillset(etterna::Skillset8::Overall) => {
			format!("{}'s Top {}", username, limit)
		}
		SkillOrAcc::Skillset(skillset) => format!("{}'s Top {} {}", username, limit, skillset),
		SkillOrAcc::Accuracy => format!("{}'s Top {} Accuracy", username, limit),
	};

	respond_score_list(ctx, &username, &title, scores).await?;

	Ok(())
}

/// Show a list of recent scores
#[poise::command(prefix_command, aliases("ls"), track_edits, slash_command)]
pub async fn lastsession(
	ctx: Context<'_>,
	#[description = "Falls back to your username"] username: Option<String>,
) -> Result<(), Error> {
	let _typing = ctx.defer_or_broadcast().await;

	let username = match username {
		Some(x) => x,
		None => ctx.data().get_eo_username(ctx.author()).await?,
	};

	let scores = ctx
		.data()
		.eo2
		.scores(
			&username,
			eo2::ScoresRequest {
				ordering: eo2::ScoresOrdering::DatetimeDescending,
				limit: Some(10),
				..Default::default()
			},
		)
		.await?;
	let scores = scores
		.into_iter()
		.map(|s| ScoreEntry {
			rate: s.rate,
			scorekey: s.key,
			song_name: s.song.name,
			ssr_overall: s.ssr.overall,
			wifescore: s.wife,
		})
		.collect();

	let title = &format!("{}'s Last 10 Scores", username);
	respond_score_list(ctx, &username, &title, scores).await?;

	Ok(())
}

/// Show details about a specific score from a previous score list
#[poise::command(prefix_command, aliases("detail"), track_edits, slash_command)]
pub async fn details(
	ctx: Context<'_>,
	#[description = "Number of the score"] position: usize,
	#[description = "Specific judge to use for statistics"] judge: Option<super::Judge>,
) -> Result<(), Error> {
	let _typing = ctx.defer_or_broadcast().await;

	let (scorekey, username) = {
		let data = ctx.data().lock_data();

		let scores_list = data
			.last_scores_list
			.get(&ctx.channel_id())
			.ok_or_else(|| anyhow::anyhow!("No score list has been posted in this channel"))?;

		let scorekey = position
			.checked_sub(1)
			.and_then(|i| scores_list.scorekeys.get(i))
			.ok_or_else(|| {
				anyhow::anyhow!("Enter a number between 1-{}", scores_list.scorekeys.len())
			})?;
		(scorekey.clone(), scores_list.username.clone())
	};

	crate::send_score_card(
		ctx,
		crate::ScoreCard {
			alternative_judge: judge.map(|x| x.0),
			scorekey: &scorekey,
			show_ssrs_and_judgements_and_modifiers: true,
			user_id: None,
			username: Some(&username),
			draw_mean_instead_of_wifescore: false,
		},
	)
	.await?;

	Ok(())
}
