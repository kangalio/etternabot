mod render;

use super::State;
use crate::{serenity, Error};

// usernames slice must contain at least one element!
fn skillgraph_inner(
	state: &State,
	ctx: &serenity::Context,
	channel_id: serenity::ChannelId,
	usernames: &[&str], // future me: leave this as is, changing it to be type-safe is ugly
) -> Result<(), Error> {
	assert!(usernames.len() >= 1);

	if usernames.len() > 20 {
		channel_id.say(
			&ctx.http,
			"Relax, now. 20 simultaneous skillgraphs ought to be enough",
		)?;
		return Ok(());
	}

	match usernames {
		[username] => channel_id.say(
			&ctx.http,
			format!("Requesting data for {} (this may take a while)", username,),
		)?,
		[usernames @ .., last] => channel_id.say(
			&ctx.http,
			format!(
				"Requesting data for {} and {} (this may take a while)",
				usernames.join(", "),
				last,
			),
		)?,
		[] => unreachable!(),
	};

	fn download_skill_timeline<'a>(
		username: &str,
		web_session: &etternaonline_api::web::Session,
		storage: &'a mut Option<etternaonline_api::web::UserScores>,
	) -> Result<etterna::SkillTimeline<&'a str>, Error> {
		let user_id = web_session.user_details(&username)?.user_id;
		let scores = web_session.user_scores(
			user_id,
			..,
			None,
			etternaonline_api::web::UserScoresSortBy::Date,
			etternaonline_api::web::SortDirection::Ascending,
			false, // exclude invalid
		)?;

		*storage = Some(scores);
		let scores = storage.as_ref().expect("impossible");

		Ok(etterna::SkillTimeline::calculate(
			scores.scores.iter().filter_map(|score| {
				Some((
					score.date.as_str(),
					score.validity_dependant.as_ref()?.ssr.to_skillsets7(),
				))
			}),
			false,
		))
	}

	const MAX_SIMULTANEOUS_DOWNLOADS: usize = 3;

	let mut storages = (0..usernames.len()).map(|_| None).collect::<Vec<_>>();
	let mut skill_timelines = Vec::with_capacity(usernames.len());
	for (username_chunk, storage_chunk) in usernames
		.chunks(MAX_SIMULTANEOUS_DOWNLOADS)
		.zip(storages.chunks_mut(MAX_SIMULTANEOUS_DOWNLOADS))
	{
		let join_handles = username_chunk
			.iter()
			.zip(storage_chunk)
			.map(|(username, storage)| {
				// SAFETY: this is safe as long as the returned handle is not leaked, which we're not doing
				unsafe {
					thread_scoped::scoped(move || {
						download_skill_timeline(username, &state.web_session, storage)
					})
				}
			})
			.collect::<Vec<_>>();

		for join_handle in join_handles {
			skill_timelines.push(join_handle.join()?);
		}
	}

	if skill_timelines.len() == 1 {
		render::draw_skillsets_graph(&skill_timelines[0], "output.png")
			.map_err(|e| e.to_string())?;
	} else {
		render::draw_user_overalls_graph(&skill_timelines, &usernames, "output.png")
			.map_err(|e| e.to_string())?;
	}

	channel_id.send_files(&ctx.http, vec!["output.png"], |m| m)?;

	Ok(())
}

pub fn skillgraph(
	state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
	args: &str,
) -> Result<(), Error> {
	let usernames = args.split_whitespace().collect::<Vec<_>>();
	if usernames.len() == 0 {
		skillgraph_inner(
			state,
			ctx,
			msg.channel_id,
			&[&state.get_eo_username(ctx, msg)?],
		)
	} else {
		skillgraph_inner(state, ctx, msg.channel_id, &usernames)
	}
}

pub fn rivalgraph(
	state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
	_args: &str,
) -> Result<(), Error> {
	let me = state.get_eo_username(ctx, msg)?;
	let you = match state.lock_data().rival(msg.author.id.0) {
		Some(rival) => rival.to_owned(),
		None => {
			msg.channel_id
				.say(&ctx.http, "Set your rival first with `+rivalset USERNAME`")?;
			return Ok(());
		}
	};
	skillgraph_inner(state, ctx, msg.channel_id, &[&me, &you])?;

	Ok(())
}

// TODO: integrate into skillgraph_inner to not duplicate logic
pub fn accuracygraph(
	state: &State,
	ctx: &serenity::Context,
	msg: &serenity::Message,
	args: &str,
) -> Result<(), Error> {
	let username = if args.is_empty() {
		state.get_eo_username(ctx, msg)?
	} else {
		args.to_owned()
	};

	msg.channel_id.say(
		&ctx.http,
		format!("Requesting data for {} (this may take a while)", username),
	)?;

	let scores = state.web_session.user_scores(
		state.web_session.user_details(&username)?.user_id,
		..,
		None,
		etternaonline_api::web::UserScoresSortBy::Date,
		etternaonline_api::web::SortDirection::Ascending,
		false, // exclude invalid
	)?;

	fn calculate_skill_timeline(
		scores: &etternaonline_api::web::UserScores,
		threshold: Option<etterna::Wifescore>,
	) -> etterna::SkillTimeline<&str> {
		etterna::SkillTimeline::calculate(
			scores.scores.iter().filter_map(|score| {
				if let Some(threshold) = threshold {
					if score.wifescore < threshold {
						return None;
					}
				}
				Some((
					score.date.as_str(),
					score.validity_dependant.as_ref()?.ssr.to_skillsets7(),
				))
			}),
			false,
		)
	}

	let full_timeline = calculate_skill_timeline(&scores, None);
	let aaa_timeline = calculate_skill_timeline(&scores, Some(etterna::Wifescore::AAA_THRESHOLD));
	let aaaa_timeline = calculate_skill_timeline(&scores, Some(etterna::Wifescore::AAAA_THRESHOLD));

	render::draw_accuracy_graph(&full_timeline, &aaa_timeline, &aaaa_timeline, "output.png")
		.map_err(|e| e.to_string())?;

	msg.channel_id
		.send_files(&ctx.http, vec!["output.png"], |m| m)?;

	Ok(())
}
