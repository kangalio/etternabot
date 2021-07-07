mod render;

use super::PrefixContext;
use crate::Error;

#[derive(Debug)]
pub struct StringError(String);
impl std::fmt::Display for StringError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&self.0)
	}
}
impl std::error::Error for StringError {}
impl From<String> for StringError {
	fn from(s: String) -> Self {
		Self(s)
	}
}

fn parse_wifescore_or_grade(string: &str) -> Option<etterna::Wifescore> {
	match &*string.to_ascii_lowercase() {
		"aaaa" => return Some(etterna::Wifescore::AAAA_THRESHOLD),
		"aaa" => return Some(etterna::Wifescore::AAA_THRESHOLD),
		"aa" => return Some(etterna::Wifescore::AA_THRESHOLD),
		"a" => return Some(etterna::Wifescore::A_THRESHOLD),
		"b" => return Some(etterna::Wifescore::B_THRESHOLD),
		"c" => return Some(etterna::Wifescore::C_THRESHOLD),
		_ => {}
	};

	etterna::Wifescore::from_percent(string.trim_end_matches('%').parse().ok()?)
}

pub struct SkillgraphConfig {
	threshold: Option<etterna::Wifescore>,
}

impl<'a> poise::PopArgument<'a> for SkillgraphConfig {
	type Err = StringError;

	fn pop_from(args: &poise::ArgString<'a>) -> Result<(poise::ArgString<'a>, Self), Self::Err> {
		let (args, params) = match poise::KeyValueArgs::pop_from(args) {
			Ok(x) => x,
			Err(e) => match e {},
		};

		let threshold = match params.get("threshold") {
			Some(string) => Some(
				parse_wifescore_or_grade(string)
					.ok_or_else(|| format!("Unknown wifescore or grade `{}`", string))?,
			),
			None => None,
		};

		Ok((args, Self { threshold }))
	}
}

// usernames slice must contain at least one element!
async fn skillgraph_inner(
	ctx: PrefixContext<'_>,
	mode: SkillgraphConfig,
	usernames: &[&str], // future me: leave this as is, changing it to be type-safe is ugly
) -> Result<(), Error> {
	assert!(usernames.len() >= 1);

	if usernames.len() > 20 {
		poise::say_prefix_reply(
			ctx,
			"Relax, now. 20 simultaneous skillgraphs ought to be enough".into(),
		)
		.await?;
		return Ok(());
	}

	match usernames {
		[username] => {
			poise::say_prefix_reply(
				ctx,
				format!("Requesting data for {} (this may take a while)", username,),
			)
			.await?;
		}
		[usernames @ .., last] => {
			poise::say_prefix_reply(
				ctx,
				format!(
					"Requesting data for {} and {} (this may take a while)",
					usernames.join(", "),
					last,
				),
			)
			.await?
		}
		[] => unreachable!(),
	};

	#[allow(clippy::needless_lifetimes)] // false positive
	async fn download_skill_timeline<'a>(
		username: &str,
		web_session: &etternaonline_api::web::Session,
		storage: &'a mut Option<etternaonline_api::web::UserScores>,
		threshold: Option<etterna::Wifescore>,
	) -> Result<etterna::SkillTimeline<&'a str>, Error> {
		let user_id = web_session.user_details(&username).await?.user_id;
		let scores = web_session
			.user_scores(
				user_id,
				..,
				None,
				etternaonline_api::web::UserScoresSortBy::Date,
				etternaonline_api::web::SortDirection::Ascending,
				false, // exclude invalid
			)
			.await?;

		*storage = Some(scores);
		let scores = storage.as_ref().expect("impossible");

		Ok(etterna::SkillTimeline::calculate(
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
		))
	}

	use futures::{StreamExt, TryStreamExt};

	let mut storages: Vec<Option<etternaonline_api::web::UserScores>> =
		(0..usernames.len()).map(|_| None).collect::<Vec<_>>();
	let skill_timelines = futures::stream::iter(usernames.iter().copied().zip(&mut storages))
		.then(|(username, storage)| {
			download_skill_timeline(username, &ctx.data.web, storage, mode.threshold)
		})
		// uncommenting this borks Rust's async :/
		// .buffered(3) // have up to three parallel connections
		.try_collect::<Vec<_>>()
		.await?;

	if skill_timelines.len() == 1 {
		render::draw_skillsets_graph(&skill_timelines[0], "output.png")
			.map_err(|e| e.to_string())?;
	} else {
		render::draw_user_overalls_graph(&skill_timelines, &usernames, "output.png")
			.map_err(|e| e.to_string())?;
	}

	// TODO, THE PROBLEM: we need some command type agnostic way of sending a message that _also_
	// supports file attachments. This needs to somehow be separate from the
	// edit-tracking-supportive message send, because edit-tracking and file attachments are not
	// compatible... Maybe a `say` variant (instead of `say_reply`) that doesn't do edit tracking
	// but in turn supports attachments etc?
	ctx.msg
		.channel_id
		.send_files(ctx.discord, vec!["output.png"], |m| m)
		.await?;

	Ok(())
}

#[poise::command]
pub async fn skillgraph(
	ctx: PrefixContext<'_>,
	mode: SkillgraphConfig,
	usernames: Vec<String>,
) -> Result<(), Error> {
	if usernames.len() == 0 {
		skillgraph_inner(
			ctx,
			mode,
			&[&ctx.data.get_eo_username(&ctx.msg.author).await?],
		)
		.await
	} else {
		let usernames = usernames.iter().map(|s| s.as_str()).collect::<Vec<_>>();
		skillgraph_inner(ctx, mode, &usernames).await
	}
}

#[poise::command]
pub async fn rivalgraph(ctx: PrefixContext<'_>, mode: SkillgraphConfig) -> Result<(), Error> {
	let me = ctx.data.get_eo_username(&ctx.msg.author).await?;
	let rival = ctx
		.data
		.lock_data()
		.rival(ctx.msg.author.id)
		.map(|x| x.to_owned());
	let you = match rival {
		Some(rival) => rival,
		None => {
			poise::say_prefix_reply(ctx, "Set your rival first with `+rivalset USERNAME`".into())
				.await?;
			return Ok(());
		}
	};
	skillgraph_inner(ctx, mode, &[&me, &you]).await?;

	Ok(())
}

// TODO: integrate into skillgraph_inner to not duplicate logic
#[poise::command(aliases("accgraph"))]
pub async fn accuracygraph(ctx: PrefixContext<'_>, username: Option<String>) -> Result<(), Error> {
	let username = match username {
		Some(x) => x,
		None => ctx.data.get_eo_username(&ctx.msg.author).await?,
	};

	ctx.msg
		.channel_id
		.say(
			ctx.discord,
			format!("Requesting data for {} (this may take a while)", username),
		)
		.await?;

	let scores = ctx
		.data
		.web
		.user_scores(
			ctx.data.web.user_details(&username).await?.user_id,
			..,
			None,
			etternaonline_api::web::UserScoresSortBy::Date,
			etternaonline_api::web::SortDirection::Ascending,
			false, // exclude invalid
		)
		.await?;

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

	ctx.msg
		.channel_id
		.send_files(ctx.discord, vec!["output.png"], |m| m)
		.await?;

	Ok(())
}
