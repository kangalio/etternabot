//! All commands that show a skill graph image

mod render;

use poise::serenity_prelude as serenity;

use crate::{Context, Error};

fn parsedate(string: &str) -> chrono::NaiveDate {
	chrono::NaiveDate::parse_from_str(string.trim(), "%Y-%m-%d %H:%M:%S")
		.expect("Invalid date from EO")
}

#[derive(Debug)]
pub struct StringError(std::borrow::Cow<'static, str>);
impl std::fmt::Display for StringError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&self.0)
	}
}
impl std::error::Error for StringError {}
impl From<String> for StringError {
	fn from(s: String) -> Self {
		Self(std::borrow::Cow::Owned(s))
	}
}
impl From<&'static str> for StringError {
	fn from(s: &'static str) -> Self {
		Self(std::borrow::Cow::Borrowed(s))
	}
}

fn parse_wifescore_or_grade(string: &str) -> Option<etterna::Wifescore> {
	match &*string.to_ascii_lowercase() {
		"aaaaa" => return Some(etterna::Wifescore::AAAAA_THRESHOLD),
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

#[derive(Clone, Copy)]
pub struct SkillgraphThreshold(Option<etterna::Wifescore>);

#[serenity::async_trait]
impl<'a> poise::PopArgument<'a> for SkillgraphThreshold {
	async fn pop_from(
		args: &'a str,
		attachment_index: usize,
		ctx: &serenity::Context,
		msg: &serenity::Message,
	) -> Result<(&'a str, usize, Self), (Box<dyn std::error::Error + Send + Sync>, Option<String>)>
	{
		let (args, attachment_index, params) =
			poise::KeyValueArgs::pop_from(args, attachment_index, ctx, msg).await?;

		let threshold = if let Some(threshold_str) = params.get("threshold") {
			Some(parse_wifescore_or_grade(threshold_str).ok_or_else(|| {
				(
					"unknown wifescore or grade".into(),
					Some(threshold_str.to_string()),
				)
			})?)
		} else {
			None
		};

		Ok((args, attachment_index, Self(threshold)))
	}
}

#[serenity::async_trait]
impl poise::SlashArgument for SkillgraphThreshold {
	async fn extract(
		ctx: &serenity::Context,
		interaction: poise::ApplicationCommandOrAutocompleteInteraction<'_>,
		value: &serde_json::Value,
	) -> Result<Self, poise::SlashArgError> {
		let threshold = if let Ok(threshold_str) =
			poise::extract_slash_argument!(String, ctx, interaction, value).await
		{
			Some(parse_wifescore_or_grade(&threshold_str).ok_or_else(|| {
				poise::SlashArgError::Parse {
					error: "unknown wifescore or grade".into(),
					input: threshold_str,
				}
			})?)
		} else {
			None
		};

		Ok(Self(threshold))
	}

	fn create(builder: &mut serenity::CreateApplicationCommandOption) {
		poise::create_slash_argument!(String, builder);
	}

	fn choices() -> Vec<poise::CommandParameterChoice> {
		Vec::new()
	}
}

// Format multiple strings ["a", "b", "c"] into a single string "a, b and c"
fn format_name_list(names: &[&str]) -> String {
	match names {
		[] => String::new(),
		[name] => name.to_string(),
		[names @ .., last_name] => format!("{} and {}", names.join(", "), last_name),
	}
}

async fn generic_download_timelines<T>(
	ctx: Context<'_>,
	usernames: &[&str],
	f: impl Fn(&str, &[eo2::Score]) -> T,
) -> Result<Vec<T>, Error> {
	assert!(usernames.len() >= 1);

	if usernames.len() > 20 {
		return Err(anyhow::anyhow!("Relax, now. 20 users ought to be enough"));
	}

	let wait_msg = format!(
		"Requesting data for {} (this may take a while)",
		format_name_list(usernames)
	);
	poise::say_reply(ctx, wait_msg).await?;

	#[allow(clippy::needless_lifetimes)] // false positive
	async fn download_timeline<'a, T>(
		username: &str,
		eo2: &eo2::Client,
		storage: &'a mut Option<Vec<eo2::Score>>,
		f: impl Fn(&str, &[eo2::Score]) -> T,
	) -> Result<T, Error> {
		let scores = eo2.scores(username, eo2::ScoresRequest::default()).await?;

		*storage = Some(scores);
		let scores = storage.as_ref().expect("impossible");

		Ok(f(username, &scores))
	}

	use futures::{StreamExt, TryStreamExt};

	let mut storages: Vec<Option<Vec<eo2::Score>>> =
		(0..usernames.len()).map(|_| None).collect::<Vec<_>>();
	let timelines = futures::stream::iter(usernames.iter().copied().zip(&mut storages))
		.then(|(username, storage)| download_timeline(username, &ctx.data().eo2, storage, &f))
		// uncommenting this borks Rust's async :/
		// .buffered(3) // have up to three parallel connections
		.try_collect::<Vec<_>>()
		.await?;

	Ok(timelines)
}

// usernames slice must contain at least one element!
async fn skillgraph_inner(
	ctx: Context<'_>,
	threshold: SkillgraphThreshold,
	usernames: &[&str], // future me: leave this as is, changing it to be type-safe is ugly
) -> Result<(), Error> {
	assert!(usernames.len() >= 1);

	let skill_timelines = generic_download_timelines(ctx, usernames, |_, scores| {
		etterna::SkillTimeline::calculate(
			scores.iter().filter_map(|score| {
				if let Some(threshold) = threshold.0 {
					if score.wife < threshold {
						return None;
					}
				}

				Some((parsedate(&score.datetime), score.ssr.skillsets7()))
			}),
			false,
		)
	})
	.await?;

	if skill_timelines.len() == 1 {
		render::draw_skillsets_graph(&skill_timelines[0], "output.png")
			.map_err(|e| anyhow::anyhow!(e))?;
	} else {
		render::draw_user_overalls_graph(&skill_timelines, &usernames, "output.png")
			.map_err(|e| anyhow::anyhow!(e))?;
	}

	poise::send_reply(ctx, |f| f.attachment("output.png".into())).await?;

	Ok(())
}

/// Show a graph of your profile rating over time
#[poise::command(prefix_command, slash_command, track_edits)]
pub async fn skillgraph(
	ctx: Context<'_>,
	#[description = "Threshold for scores to be included in the calculation"]
	threshold: SkillgraphThreshold,
	#[description = "Which user to show"] usernames: Vec<String>,
) -> Result<(), Error> {
	let _typing = ctx.defer_or_broadcast().await;

	if usernames.len() == 0 {
		skillgraph_inner(
			ctx,
			threshold,
			&[&ctx.data().get_eo_username(ctx.author()).await?],
		)
		.await
	} else {
		let usernames = usernames.iter().map(|s| s.as_str()).collect::<Vec<_>>();
		skillgraph_inner(ctx, threshold, &usernames).await
	}
}

/// Show a graph of your profile versus your rival's profile rating over time
#[poise::command(prefix_command, slash_command, track_edits)]
pub async fn rivalgraph(
	ctx: Context<'_>,
	#[description = "Threshold for scores to be included in the calculation"]
	threshold: SkillgraphThreshold,
) -> Result<(), Error> {
	let _typing = ctx.defer_or_broadcast().await;

	let me = ctx.data().get_eo_username(ctx.author()).await?;
	let rival = ctx
		.data()
		.lock_data()
		.rival(ctx.author().id)
		.map(|x| x.to_owned());
	let you = match rival {
		Some(rival) => rival,
		None => {
			poise::say_reply(ctx, "Set your rival first with `+rivalset USERNAME`").await?;
			return Ok(());
		}
	};
	skillgraph_inner(ctx, threshold, &[&me, &you]).await?;

	Ok(())
}

// TODO: integrate into skillgraph_inner to not duplicate logic
/// Calculate your profile rating over time, considering only scores above a certain threshold
#[poise::command(prefix_command, slash_command, track_edits, aliases("accgraph"))]
pub async fn accuracygraph(
	ctx: Context<'_>,
	#[description = "Profile to show"]
	#[autocomplete = "crate::autocomplete_username"]
	username: Option<String>,
) -> Result<(), Error> {
	let _typing = ctx.defer_or_broadcast().await;

	let username = match username {
		Some(x) => x,
		None => ctx.data().get_eo_username(ctx.author()).await?,
	};

	poise::say_reply(
		ctx,
		format!("Requesting data for {} (this may take a while)", username),
	)
	.await?;

	let scores = ctx
		.data()
		.eo2
		.scores(&username, eo2::ScoresRequest::default())
		.await?;

	fn calculate_skill_timeline(
		scores: &[eo2::Score],
		threshold: Option<etterna::Wifescore>,
	) -> etterna::SkillTimeline<&str> {
		etterna::SkillTimeline::calculate(
			scores.iter().filter_map(|score| {
				if let Some(threshold) = threshold {
					if score.wife < threshold {
						return None;
					}
				}
				Some((&*score.datetime, score.ssr.skillsets7()))
			}),
			false,
		)
	}

	let full_timeline = calculate_skill_timeline(&scores, None);
	let aaa_timeline = calculate_skill_timeline(&scores, Some(etterna::Wifescore::AAA_THRESHOLD));
	let aaaa_timeline = calculate_skill_timeline(&scores, Some(etterna::Wifescore::AAAA_THRESHOLD));
	let aaaaa_timeline =
		calculate_skill_timeline(&scores, Some(etterna::Wifescore::AAAAA_THRESHOLD));

	render::draw_accuracy_graph(
		&full_timeline,
		&aaa_timeline,
		&aaaa_timeline,
		&aaaaa_timeline,
		"output.png",
	)
	.map_err(|e| anyhow::anyhow!(e))?;

	let mut content = format!(
		"Full rating: **{:.2}**",
		full_timeline
			.changes
			.last()
			.map_or(0.0, |(_, rating)| rating.overall),
	);
	for &(timeline, name) in &[
		(&aaa_timeline, "AAA-only rating"),
		(&aaaa_timeline, "AAAA-only rating"),
		(&aaaaa_timeline, "AAAAA-only rating"),
	] {
		if let Some((_, rating)) = timeline.changes.last() {
			content += &format!("\n{}: **{:.2}**", name, rating.overall);
		}
	}
	poise::send_reply(ctx, |f| f.content(content).attachment("output.png".into())).await?;

	Ok(())
}

/// Show a graph of your total number of scores over time
#[poise::command(prefix_command, slash_command, track_edits)]
pub async fn scoregraph(
	ctx: Context<'_>,
	#[description = "Which users to include in the graph"] usernames: Vec<String>,
) -> Result<(), Error> {
	let _typing = ctx.defer_or_broadcast().await;

	let usernames: Vec<String> = if usernames.is_empty() {
		vec![ctx.data().get_eo_username(ctx.author()).await?]
	} else {
		usernames
	};
	let usernames: Vec<&str> = usernames.iter().map(|x| x.as_str()).collect();

	fn calculate_timeline(
		scores: &[eo2::Score],
		range: std::ops::Range<etterna::Wifescore>,
	) -> Vec<(chrono::NaiveDate, u32)> {
		use itertools::Itertools;

		let mut num_total_scores = 0;
		scores
			.iter()
			.filter(|s| range.contains(&s.wife))
			.group_by(|s| s.datetime.as_str())
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
			sub_aa_timeline: if usernames.len() == 1 {
				Some(calculate_timeline(
					&scores,
					etterna::Wifescore::from_percent(50.0).unwrap()
						..etterna::Wifescore::AA_THRESHOLD,
				))
			} else {
				None
			},
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
			aaaaa_timeline: calculate_timeline(
				&scores,
				etterna::Wifescore::AAAAA_THRESHOLD..etterna::Wifescore::HUNDRED_PERCENT,
			),
		})
		.await?;

	render::draw_score_graph(&score_timelines, "output.png").map_err(|e| anyhow::anyhow!(e))?;

	poise::send_reply(ctx, |f| {
		f.attachment("output.png".into());
		if let [user] = &*score_timelines {
			let mut content = format!(
				"Number of sub-AAs: **{}**\nNumber of AAs: **{}**\nNumber of AAAs: **{}**\nNumber of AAAAs: **{}**\n",
				match &user.sub_aa_timeline {
					Some(x) => x.last().map_or(0, |&(_, total)| total),
					None => 0, // shouldn't happen
				},
				user.aa_timeline.last().map_or(0, |&(_, total)| total),
				user.aaa_timeline.last().map_or(0, |&(_, total)| total),
				user.aaaa_timeline.last().map_or(0, |&(_, total)| total),
			);
			if let Some((_, num_aaaaas)) = user.aaaaa_timeline.last() {
				content += &format!("Number of AAAAAs: **{}**\n", num_aaaaas);
			}
			f.content(content);
		}
		f
	}).await?;

	Ok(())
}
