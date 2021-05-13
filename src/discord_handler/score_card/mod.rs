//! Utility code used by various parts of the bot to show a score card

mod replay_graph;

use super::State;
use crate::{serenity, Error};

pub struct ScoreCard<'a> {
	pub scorekey: &'a etterna::Scorekey,
	pub user_id: Option<u32>, // pass None if score link shouldn't be shown
	pub show_ssrs_and_judgements_and_modifiers: bool,
	pub alternative_judge: Option<&'a etterna::Judge>,
}

pub async fn send_score_card(
	state: &State,
	ctx: &serenity::Context,
	channel: serenity::ChannelId,
	info: ScoreCard<'_>,
) -> Result<(), Error> {
	let message = score_card_inner(state, info).await?;
	channel
		.send_message(ctx, |m| {
			*m = message;
			m
		})
		.await?;
	Ok(())
}

async fn score_card_inner(
	state: &State,
	info: ScoreCard<'_>,
) -> Result<serenity::CreateMessage<'static>, Error> {
	let score = state.v2().await?.score_data(info.scorekey).await?;

	let alternative_judge_wifescore = if let Some(alternative_judge) = info.alternative_judge {
		if let Some(replay) = &score.replay {
			etterna::rescore_from_note_hits::<etterna::Wife3, _>(
				replay.notes.iter().map(|note| note.hit),
				score.judgements.hit_mines,
				score.judgements.let_go_holds + score.judgements.missed_holds,
				alternative_judge,
			)
		} else {
			None
		}
	} else {
		None
	};

	let mut description = String::new();
	if let Some(user_id) = info.user_id {
		description += &format!(
			"https://etternaonline.com/score/view/{}{}\n",
			info.scorekey, user_id
		);
	}
	if info.show_ssrs_and_judgements_and_modifiers {
		description += &format!("```\n{}\n```", score.modifiers);
	}
	description += &format!(
		r#"```nim
{}
   Max Combo: {:<5.0}   ⏐        Perfect: {}
     Overall: {:<5.2}   ⏐          Great: {}
      Stream: {:<5.2}   ⏐           Good: {}
     Stamina: {:<5.2}   ⏐            Bad: {}
  Jumpstream: {:<5.2}   ⏐           Miss: {}
  Handstream: {:<5.2}   ⏐      Hit Mines: {}
       Jacks: {:<5.2}   ⏐     Held Holds: {}
   Chordjack: {:<5.2}   ⏐  Dropped Holds: {}
   Technical: {:<5.2}   ⏐   Missed Holds: {}
```
"#,
		if let Some(alternative_judge_wifescore) = alternative_judge_wifescore {
			format!(
				concat!(
					"        Wife: {:<5.2}%  ⏐\n",
					"     Wife {}: {:<5.2}%  ⏐      Marvelous: {}",
				),
				score.wifescore.as_percent(),
				// UWNRAP: if alternative_judge_wifescore is Some, info.alternative_judge is too
				info.alternative_judge.unwrap().name,
				alternative_judge_wifescore.as_percent(),
				score.judgements.marvelouses,
			)
		} else {
			format!(
				"        Wife: {:<5.2}%  ⏐      Marvelous: {}",
				score.wifescore.as_percent(),
				score.judgements.marvelouses,
			)
		},
		score.max_combo,
		score.judgements.perfects,
		score.ssr.overall,
		score.judgements.greats,
		score.ssr.stream,
		score.judgements.goods,
		score.ssr.stamina,
		score.judgements.bads,
		score.ssr.jumpstream,
		score.judgements.misses,
		score.ssr.handstream,
		score.judgements.hit_mines,
		score.ssr.jackspeed,
		score.judgements.held_holds,
		score.ssr.chordjack,
		score.judgements.let_go_holds,
		score.ssr.technical,
		score.judgements.missed_holds,
	);

	struct ScoringSystemComparison {
		wife2_score: etterna::Wifescore,
		wife3_score: etterna::Wifescore,
		wife3_score_zero_mean: etterna::Wifescore,
	}

	struct ReplayAnalysis {
		replay_graph_path: &'static str,
		scoring_system_comparison_j4: ScoringSystemComparison,
		scoring_system_comparison_alternative: Option<ScoringSystemComparison>,
		fastest_finger_jackspeed: f32, // NPS, single finger
		fastest_nps: f32,
		longest_100_combo: u32,
		longest_marv_combo: u32,
		longest_perf_combo: u32,
		longest_combo: u32,
		mean_offset: f32,
	}

	let do_replay_analysis =
		|score: &etternaonline_api::v2::ScoreData| -> Option<Result<ReplayAnalysis, Error>> {
			use etterna::SimpleReplay;

			let replay = score.replay.as_ref()?;

			let r = replay_graph::generate_replay_graph(replay, "replay_graph.png").transpose()?;
			if let Err(e) = r {
				return Some(Err(e.into()));
			}

			// in the following, DONT scale find_fastest_note_subset results by rate - I only needed
			// to do that for etterna-graph where the note seconds where unscaled. EO's note seconds
			// _are_ scaled though.

			let lanes = replay.split_into_lanes()?;
			let mut max_finger_nps = 0.0;
			for lane in &lanes {
				let this_fingers_max_nps =
					etterna::find_fastest_note_subset(&lane.hit_seconds, 20, 20).speed;

				if this_fingers_max_nps > max_finger_nps {
					max_finger_nps = this_fingers_max_nps;
				}
			}

			let note_and_hit_seconds = replay.split_into_notes_and_hits()?;
			let unsorted_hit_seconds = note_and_hit_seconds.hit_seconds;

			let mut sorted_hit_seconds = unsorted_hit_seconds;
			// UNWRAP: if one of those values is NaN... something is pretty wrong
			sorted_hit_seconds.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
			let sorted_hit_seconds = sorted_hit_seconds;

			let fastest_nps =
				etterna::find_fastest_note_subset(&sorted_hit_seconds, 100, 100).speed;

			let mean_offset = replay.mean_deviation();
			let replay_zero_mean = etternaonline_api::Replay {
				notes: replay
					.notes
					.iter()
					.map(|note| {
						let mut note = note.clone();
						if let etterna::Hit::Hit { deviation } = &mut note.hit {
							*deviation -= mean_offset;
						}
						note
					})
					.collect(),
			};

			Some(Ok(ReplayAnalysis {
				replay_graph_path: "replay_graph.png",
				scoring_system_comparison_j4: ScoringSystemComparison {
					wife2_score: etternaonline_api::rescore::<etterna::NaiveScorer, etterna::Wife2>(
						replay,
						score.judgements.hit_mines,
						score.judgements.let_go_holds + score.judgements.missed_holds,
						&etterna::J4,
					)?,
					wife3_score: etternaonline_api::rescore::<etterna::NaiveScorer, etterna::Wife3>(
						replay,
						score.judgements.hit_mines,
						score.judgements.let_go_holds + score.judgements.missed_holds,
						&etterna::J4,
					)?,
					wife3_score_zero_mean: etternaonline_api::rescore::<
						etterna::NaiveScorer,
						etterna::Wife3,
					>(
						&replay_zero_mean,
						score.judgements.hit_mines,
						score.judgements.let_go_holds + score.judgements.missed_holds,
						&etterna::J4,
					)?,
				},
				scoring_system_comparison_alternative: match info.alternative_judge {
					Some(alternative_judge) => Some(ScoringSystemComparison {
						wife2_score: etternaonline_api::rescore::<
							etterna::NaiveScorer,
							etterna::Wife2,
						>(
							replay,
							score.judgements.hit_mines,
							score.judgements.let_go_holds + score.judgements.missed_holds,
							alternative_judge,
						)?,
						wife3_score: etternaonline_api::rescore::<
							etterna::NaiveScorer,
							etterna::Wife3,
						>(
							replay,
							score.judgements.hit_mines,
							score.judgements.let_go_holds + score.judgements.missed_holds,
							alternative_judge,
						)?,
						wife3_score_zero_mean: etternaonline_api::rescore::<
							etterna::NaiveScorer,
							etterna::Wife3,
						>(
							&replay_zero_mean,
							score.judgements.hit_mines,
							score.judgements.let_go_holds + score.judgements.missed_holds,
							alternative_judge,
						)?,
					}),
					None => None,
				},
				fastest_finger_jackspeed: max_finger_nps,
				fastest_nps,
				longest_100_combo: replay.longest_combo(|hit| hit.is_within_window(0.005)),
				longest_marv_combo: replay
					.longest_combo(|hit| hit.is_within_window(etterna::J4.marvelous_window)),
				longest_perf_combo: replay
					.longest_combo(|hit| hit.is_within_window(etterna::J4.perfect_window)),
				longest_combo: replay
					.longest_combo(|hit| hit.is_within_window(etterna::J4.great_window)),
				mean_offset,
			}))
		};

	let replay_analysis = do_replay_analysis(&score).transpose()?;

	let mut embed = serenity::CreateEmbed::default();
	embed
		.color(crate::ETTERNA_COLOR)
		.author(|a| {
			a.name(&score.song_name)
				.url(format!(
					"https://etternaonline.com/song/view/{}",
					score.song_id
				))
				.icon_url(format!(
					"https://etternaonline.com/img/flags/{}.png",
					score.user.country_code
				))
		})
		// .thumbnail(format!("https://etternaonline.com/avatars/{}", score.user.avatar)) // takes too much space
		.description(description)
		.footer(|f| {
			f.text(format!("Played by {}", &score.user.username))
				.icon_url(format!(
					"https://etternaonline.com/avatars/{}",
					score.user.avatar
				))
		});

	if let Some(analysis) = &replay_analysis {
		let wifescore_floating_point_digits = match analysis
			.scoring_system_comparison_j4
			.wife3_score
			.as_percent()
			> 99.7
		{
			true => 4,
			false => 2,
		};

		let alternative_text_1;
		let alternative_text_2;
		let alternative_text_4;
		if let Some(comparison) = &analysis.scoring_system_comparison_alternative {
			// UNWRAP: if we're in this branch, info.alternative_judge is Some
			alternative_text_1 = format!(
				", {:.digits$} on {}",
				comparison.wife2_score,
				info.alternative_judge.unwrap().name,
				digits = wifescore_floating_point_digits,
			);
			alternative_text_2 = format!(
				", {:.digits$} on {}",
				comparison.wife3_score,
				info.alternative_judge.unwrap().name,
				digits = wifescore_floating_point_digits,
			);
			alternative_text_4 = format!(
				", {:.digits$} on {}",
				comparison.wife3_score_zero_mean,
				info.alternative_judge.unwrap().name,
				digits = wifescore_floating_point_digits,
			);
		} else {
			alternative_text_1 = "".to_owned();
			alternative_text_2 = "".to_owned();
			alternative_text_4 = "".to_owned();
		}

		embed
			.attachment(analysis.replay_graph_path)
			.field(
				"Score comparisons",
				format!(
					concat!(
						"{}",
						"**Wife2**: {:.digits$}%{}\n",
						"**Wife3**: {:.digits$}%{}\n",
						"**Wife3**: {:.digits$}%{} (mean of {:.1}ms corrected)",
					),
					if (analysis
						.scoring_system_comparison_j4
						.wife3_score
						.as_percent() - score.wifescore.as_percent())
					.abs() > 0.01
					{
						"_Note: these calculated scores are slightly inaccurate_\n"
					} else {
						""
					},
					analysis
						.scoring_system_comparison_j4
						.wife2_score
						.as_percent(),
					alternative_text_1,
					analysis
						.scoring_system_comparison_j4
						.wife3_score
						.as_percent(),
					alternative_text_2,
					analysis
						.scoring_system_comparison_j4
						.wife3_score_zero_mean
						.as_percent(),
					alternative_text_4,
					analysis.mean_offset * 1000.0,
					digits = wifescore_floating_point_digits,
				),
				false,
			)
			.field(
				"Tap speeds",
				format!(
					"Fastest jack over a course of 20 notes: {:.2} NPS\n\
							Fastest total NPS over a course of 100 notes: {:.2} NPS",
					analysis.fastest_finger_jackspeed, analysis.fastest_nps,
				),
				false,
			)
			.field(
				"Combos",
				format!(
					"Longest combo: {}\n\
							Longest perfect combo: {}\n\
							Longest marvelous combo: {}\n\
							Longest 100% combo: {}\n",
					analysis.longest_combo,
					analysis.longest_perf_combo,
					analysis.longest_marv_combo,
					analysis.longest_100_combo,
				),
				false,
			);
	}

	let mut message = serenity::CreateMessage::default();
	message.embed(|e| {
		*e = embed;
		e
	});
	if let Some(analysis) = &replay_analysis {
		message.add_file(analysis.replay_graph_path);
	}

	Ok(message)
}
