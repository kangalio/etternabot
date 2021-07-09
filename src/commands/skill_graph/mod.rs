mod render;

use super::PrefixContext;
use crate::Error;

fn parsedate(string: &str) -> chrono::Date<chrono::Utc> {
	chrono::Date::from_utc(
		chrono::NaiveDate::parse_from_str(string.trim(), "%Y-%m-%d").expect("Invalid date from EO"),
		chrono::Utc,
	)
}

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

async fn generic_download_timelines<T>(
	ctx: PrefixContext<'_>,
	usernames: &[&str],
	f: impl Fn(&str, &[etternaonline_api::web::UserScore]) -> T,
) -> Result<Vec<T>, Error> {
	assert!(usernames.len() >= 1);

	if usernames.len() > 20 {
		return Err("Relax, now. 20 users ought to be enough".into());
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
	async fn download_timeline<'a, T>(
		username: &str,
		web_session: &etternaonline_api::web::Session,
		storage: &'a mut Option<etternaonline_api::web::UserScores>,
		f: impl Fn(&str, &[etternaonline_api::web::UserScore]) -> T,
	) -> Result<T, Error> {
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

		Ok(f(username, &scores.scores))
	}

	use futures::{StreamExt, TryStreamExt};

	let mut storages: Vec<Option<etternaonline_api::web::UserScores>> =
		(0..usernames.len()).map(|_| None).collect::<Vec<_>>();
	let timelines = futures::stream::iter(usernames.iter().copied().zip(&mut storages))
		.then(|(username, storage)| download_timeline(username, &ctx.data.web, storage, &f))
		// uncommenting this borks Rust's async :/
		// .buffered(3) // have up to three parallel connections
		.try_collect::<Vec<_>>()
		.await?;

	Ok(timelines)
}

// usernames slice must contain at least one element!
async fn skillgraph_inner(
	ctx: PrefixContext<'_>,
	mode: SkillgraphConfig,
	usernames: &[&str], // future me: leave this as is, changing it to be type-safe is ugly
) -> Result<(), Error> {
	assert!(usernames.len() >= 1);

	let skill_timelines = generic_download_timelines(ctx, usernames, |_, scores| {
		etterna::SkillTimeline::calculate(
			scores.iter().filter_map(|score| {
				if let Some(threshold) = mode.threshold {
					if score.wifescore < threshold {
						return None;
					}
				}

				Some((
					parsedate(&score.date),
					score.validity_dependant.as_ref()?.ssr.to_skillsets7(),
				))
			}),
			false,
		)
	})
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

#[poise::command]
pub async fn scoregraph(ctx: PrefixContext<'_>, usernames: Vec<String>) -> Result<(), Error> {
	let usernames: Vec<String> = if usernames.is_empty() {
		vec![ctx.data.get_eo_username(&ctx.msg.author).await?]
	} else {
		usernames
	};
	let usernames: Vec<&str> = usernames.iter().map(|x| x.as_str()).collect();

	fn calculate_timeline(
		scores: &[etternaonline_api::web::UserScore],
		range: std::ops::Range<etterna::Wifescore>,
	) -> Vec<(chrono::Date<chrono::Utc>, u32)> {
		use itertools::Itertools;

		let mut num_total_scores = 0;
		scores
			.iter()
			.filter(|s| range.contains(&s.wifescore))
			.group_by(|s| s.date.as_str())
			.into_iter()
			.map(|(day, scores)| {
				num_total_scores += scores.count() as u32;
				(parsedate(day), num_total_scores)
			})
			.collect()
	}

	let score_timelines =
		generic_download_timelines(ctx, &usernames, |username, scores| render::ScoreGraphUser {
			username: username.to_owned(),
			sub_aa_timeline: calculate_timeline(
				&scores,
				etterna::Wifescore::NEG_INFINITY..etterna::Wifescore::AA_THRESHOLD,
			),
			aa_timeline: calculate_timeline(
				&scores,
				etterna::Wifescore::AA_THRESHOLD..etterna::Wifescore::AAA_THRESHOLD,
			),
			aaa_timeline: calculate_timeline(
				&scores,
				etterna::Wifescore::AAA_THRESHOLD..etterna::Wifescore::AAAA_THRESHOLD,
			),
			aaaa_timeline: calculate_timeline(
				&scores,
				etterna::Wifescore::AAAA_THRESHOLD..etterna::Wifescore::AAAAA_THRESHOLD,
			),
		})
		.await?;

	render::draw_score_graph(&score_timelines, "output.png").map_err(|e| e.to_string())?;

	ctx.msg
		.channel_id
		.send_files(ctx.discord, vec!["output.png"], |f| {
			// Only add that text if a single user was selected
			if let [user] = &*score_timelines {
				f.content(format!(
					"Number of sub-AAs: **{}**\nNumber of AAs: **{}**\nNumber of AAAs: **{}**\nNumber of AAAAs: **{}**",
					user.sub_aa_timeline.last().map_or(0, |&(_, total)| total),
					user.aa_timeline.last().map_or(0, |&(_, total)| total),
					user.aaa_timeline.last().map_or(0, |&(_, total)| total),
					user.aaaa_timeline.last().map_or(0, |&(_, total)| total),
				));
			}
			f
		})
		.await?;

	Ok(())
}
