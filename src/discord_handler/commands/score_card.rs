//! All commands that spawn a score card

use super::PrefixContext;
use crate::Error;

pub struct Judge(&'static etterna::Judge);
impl std::str::FromStr for Judge {
	type Err = ();
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		super::extract_judge_from_string(s).map(Self).ok_or(())
	}
}

/// Call this command with `+rs [username] [judge]`
#[poise::command] // can't make slash command because it shows a score card
pub async fn rs(
	ctx: PrefixContext<'_>,
	#[lazy] eo_username: Option<String>,
	alternative_judge: Option<poise::Wrapper<Judge>>,
) -> Result<(), Error> {
	let eo_username = match eo_username {
		Some(x) => x,
		None => ctx.data.get_eo_username(&ctx.msg.author)?,
	};
	let alternative_judge = alternative_judge.map(|j| j.0 .0);

	let latest_scores = ctx.data.v2()?.user_latest_scores(&eo_username)?;
	let latest_score = match latest_scores.first() {
		Some(x) => x,
		None => {
			poise::say_prefix_reply(ctx, "User has no scores".into()).await?;
			return Ok(());
		}
	};

	let user_id = ctx.data.get_eo_user_id(&eo_username)?;
	super::send_score_card(
		ctx.data,
		ctx.discord,
		ctx.msg.channel_id,
		super::ScoreCard {
			scorekey: &latest_score.scorekey,
			user_id: Some(user_id),
			show_ssrs_and_judgements_and_modifiers: true,
			alternative_judge,
		},
	)
	.await?;

	Ok(())
}

fn get_random_score(
	registry_entry: &mut super::config::UserRegistryEntry,
	web_session: &etternaonline_api::web::Session,
) -> Result<etternaonline_api::web::UserScore, Error> {
	use rand::Rng as _;

	let scores = if let Some(last_known_num_scores) = registry_entry.last_known_num_scores {
		// choose a random score
		let score_index = rand::thread_rng().gen_range(0, last_known_num_scores);

		web_session.user_scores(
			registry_entry.eo_id,
			score_index..=score_index,
			None,
			etternaonline_api::web::UserScoresSortBy::Date, // doesnt matter
			etternaonline_api::web::SortDirection::Ascending, // doesnt matter
			true,
		)?
	} else {
		// let's get the first score by scorekey - the scorekey is pretty random, so this will seem
		// sufficiently random - at least for the first time. Doing it multiple times would yield
		// the same score every time BUT since we're writing the number of scores after this, future
		// invocations can directly request a random index
		web_session.user_scores(
			registry_entry.eo_id,
			0..1,
			None,
			etternaonline_api::web::UserScoresSortBy::Scorekey,
			etternaonline_api::web::SortDirection::Ascending,
			true,
		)?
	};

	registry_entry.last_known_num_scores = Some(scores.entries_before_search_filtering);

	scores
		.scores
		.into_iter()
		.next()
		.ok_or_else(|| "A score was requested from EO but none was sent".into())
}

#[poise::command]
pub async fn randomscore(
	ctx: PrefixContext<'_>,
	#[lazy] username: Option<String>,
	judge: Option<poise::Wrapper<Judge>>,
) -> Result<(), Error> {
	let username = match username {
		Some(x) => x,
		None => ctx.data.get_eo_username(&ctx.msg.author)?,
	};

	let (user_eo_id, scorekey) = {
		let mut data = ctx.data.lock_data();
		let user = data
			.user_registry
			.iter_mut()
			.find(|user| user.eo_username.eq_ignore_ascii_case(&username))
			.ok_or(crate::MISSING_REGISTRY_ENTRY_ERROR_MESSAGE)?;

		// find a random score. If it's invalid, find another one
		let scorekey = loop {
			let score = get_random_score(user, &ctx.data.web_session)?;
			if let Some(validity_dependant) = score.validity_dependant {
				break validity_dependant.scorekey;
			}
		};

		(user.eo_id, scorekey)
	};

	super::send_score_card(
		ctx.data,
		ctx.discord,
		ctx.msg.channel_id,
		super::ScoreCard {
			scorekey: &scorekey,
			user_id: Some(user_eo_id),
			show_ssrs_and_judgements_and_modifiers: true,
			alternative_judge: judge.map(|x| x.0 .0),
		},
	)
	.await?;

	Ok(())
}
