//! All commands that spawn a score card

use crate::{Context, Error};

#[derive(Debug)]
pub struct InvalidJudge;
impl std::fmt::Display for InvalidJudge {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str("Please write the judge like `J5` or `J8`")
	}
}
impl std::error::Error for InvalidJudge {}

pub struct Judge(pub &'static etterna::Judge);
impl std::str::FromStr for Judge {
	type Err = InvalidJudge;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		crate::extract_judge_from_string(s)
			.map(Self)
			.ok_or(InvalidJudge)
	}
}

/// Call this command with `+rs [username] [judge]`
#[poise::command(prefix_command, slash_command, track_edits)]
pub async fn rs(
	ctx: Context<'_>,
	#[lazy]
	#[description = "EtternaOnline username"]
	eo_username: Option<String>,
	#[description = "Judge to show info about"] alternative_judge: Option<Judge>,
) -> Result<(), Error> {
	let _typing = ctx.defer_or_broadcast().await;

	let eo_username = match eo_username {
		Some(x) => x,
		None => ctx.data().get_eo_username(ctx.author()).await?,
	};
	let alternative_judge = alternative_judge.map(|j| j.0);

	let user_id = ctx.data().get_eo_user_id(&eo_username).await?;

	let latest_scores = ctx
		.data()
		.web
		.user_scores(
			user_id,
			0..1,
			None,
			etternaonline_api::web::UserScoresSortBy::Date,
			etternaonline_api::web::SortDirection::Descending,
			true,
		)
		.await?
		.scores;
	let score = latest_scores.first().ok_or("User has no scores")?;
	let scorekey = &score
		.validity_dependant
		.as_ref()
		.ok_or("Latest score is invalid")?
		.scorekey;

	crate::send_score_card(
		ctx,
		crate::ScoreCard {
			scorekey,
			user_id: Some(user_id),
			username: Some(&eo_username),
			show_ssrs_and_judgements_and_modifiers: true,
			alternative_judge,
		},
	)
	.await?;

	Ok(())
}

async fn get_random_score(
	state: &crate::State,
	username: &str,
	web_session: &etternaonline_api::web::Session,
) -> Result<etternaonline_api::web::UserScore, Error> {
	use rand::Rng as _;

	let crate::config::UserRegistryEntry {
		eo_id,
		last_known_num_scores,
		..
	} = *state
		.lock_data()
		.user_registry
		.iter_mut()
		.find(|user| user.eo_username.eq_ignore_ascii_case(&username))
		.ok_or(crate::MISSING_REGISTRY_ENTRY_ERROR_MESSAGE)?;

	let scores = if let Some(last_known_num_scores) = last_known_num_scores {
		// choose a random score
		let score_index = rand::thread_rng().gen_range(0, last_known_num_scores);

		web_session
			.user_scores(
				eo_id,
				score_index..=score_index,
				None,
				etternaonline_api::web::UserScoresSortBy::Date, // doesnt matter
				etternaonline_api::web::SortDirection::Ascending, // doesnt matter
				true,
			)
			.await?
	} else {
		// let's get the first score by scorekey - the scorekey is pretty random, so this will seem
		// sufficiently random - at least for the first time. Doing it multiple times would yield
		// the same score every time BUT since we're writing the number of scores after this, future
		// invocations can directly request a random index
		web_session
			.user_scores(
				eo_id,
				0..1,
				None,
				etternaonline_api::web::UserScoresSortBy::Scorekey,
				etternaonline_api::web::SortDirection::Ascending,
				true,
			)
			.await?
	};

	if let Some(registry_entry) = state
		.lock_data()
		.user_registry
		.iter_mut()
		.find(|user| user.eo_username.eq_ignore_ascii_case(&username))
	{
		registry_entry.last_known_num_scores = Some(scores.entries_before_search_filtering);
	} else {
		log::warn!("User registry entry has disappeared while retrieving random score");
	}

	scores
		.scores
		.into_iter()
		.next()
		.ok_or_else(|| "A score was requested from EO but none was sent".into())
}

/// Show a random score
#[poise::command(prefix_command, slash_command, track_edits)]
pub async fn randomscore(
	ctx: Context<'_>,
	#[lazy]
	#[description = "EtternaOnline username"]
	username: Option<String>,
	#[description = "Judge to show info about"] judge: Option<Judge>,
) -> Result<(), Error> {
	let _typing = ctx.defer_or_broadcast().await;

	let username = match username {
		Some(x) => x,
		None => ctx.data().get_eo_username(ctx.author()).await?,
	};

	// find a random score. If it's invalid, find another one
	let (user_eo_id, scorekey) = loop {
		let score = get_random_score(ctx.data(), &username, &ctx.data().web).await?;
		if let Some(validity_dependant) = score.validity_dependant {
			break (validity_dependant.user_id, validity_dependant.scorekey);
		}
	};

	crate::send_score_card(
		ctx,
		crate::ScoreCard {
			scorekey: &scorekey,
			user_id: Some(user_eo_id),
			username: Some(&username),
			show_ssrs_and_judgements_and_modifiers: true,
			alternative_judge: judge.map(|x| x.0),
		},
	)
	.await?;

	Ok(())
}
