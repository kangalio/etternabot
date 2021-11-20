//! Utility code used by various parts of the bot to show a score card

mod replay_analysis;
mod replay_graph;

use crate::{serenity, Context, Error};

pub struct ScoreCard<'a> {
	pub scorekey: &'a etterna::Scorekey,
	pub user_id: Option<u32>,      // pass None if score link shouldn't be shown
	pub username: Option<&'a str>, // used to detect scorekey collision
	pub show_ssrs_and_judgements_and_modifiers: bool,
	pub alternative_judge: Option<&'a etterna::Judge>,
	pub draw_mean_instead_of_wifescore: bool,
}

fn write_score_card_body(
	info: &ScoreCard<'_>,
	score: &etternaonline_api::v1::ScoreData,
	alternative_judge_wifescore: Option<etterna::Wifescore>,
) -> String {
	let mut description = String::new();

	if let Some(expected_username) = info.username {
		if !score.user.username.eq_ignore_ascii_case(expected_username) {
			description += "**_Multiple scores were assigned the same unique identifier (scorekey), so you are seeing the wrong score here. Sorry!_**\n";
		}
	}

	if let Some(user_id) = info.user_id {
		description += &format!(
			"https://etternaonline.com/score/view/{}{}\n",
			info.scorekey, user_id
		);
	}

	if info.show_ssrs_and_judgements_and_modifiers {
		description += &format!("```\n{}\n```", score.modifiers);
	}

	description += "```nim\n";
	description += &if let Some(alternative_judge_wifescore) = alternative_judge_wifescore {
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
	};
	description += &format!(
		"
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
",
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

	description
}

fn generate_score_comparisons_text(
	score: &etternaonline_api::v1::ScoreData,
	analysis: &replay_analysis::ReplayAnalysis,
	alternative_judge: Option<&etterna::Judge>,
) -> String {
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
			alternative_judge.unwrap().name,
			digits = wifescore_floating_point_digits,
		);
		alternative_text_2 = format!(
			", {:.digits$} on {}",
			comparison.wife3_score,
			alternative_judge.unwrap().name,
			digits = wifescore_floating_point_digits,
		);
		alternative_text_4 = format!(
			", {:.digits$} on {}",
			comparison.wife3_score_zero_mean,
			alternative_judge.unwrap().name,
			digits = wifescore_floating_point_digits,
		);
	} else {
		alternative_text_1 = "".to_owned();
		alternative_text_2 = "".to_owned();
		alternative_text_4 = "".to_owned();
	}

	let mut score_comparisons_text = String::new();

	if (analysis
		.scoring_system_comparison_j4
		.wife3_score
		.as_percent()
		- score.wifescore.as_percent())
	.abs() > 0.01
	{
		score_comparisons_text += "_Note: these calculated scores are slightly inaccurate_\n";
	}

	score_comparisons_text += &format!(
		"\
**Wife2**: {:.digits$}%{}
**Wife3**: {:.digits$}%{}
**Wife3**: {:.digits$}%{} (mean of {:.1}ms corrected)",
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
	);

	score_comparisons_text
}

pub async fn send_score_card(ctx: Context<'_>, info: ScoreCard<'_>) -> Result<(), Error> {
	let score = ctx.data().v1.score_data(info.scorekey).await?;

	let alternative_judge_wifescore = match (info.alternative_judge, &score.replay) {
		(Some(alternative_judge), Some(replay)) => {
			etterna::rescore_from_note_hits::<etterna::Wife3, _>(
				replay.notes.iter().map(|note| note.hit),
				score.judgements.hit_mines,
				score.judgements.let_go_holds + score.judgements.missed_holds,
				alternative_judge,
			)
		}
		_ => None,
	};

	let description = write_score_card_body(&info, &score, alternative_judge_wifescore);

	let replay_analysis = replay_analysis::do_replay_analysis(
		&score,
		info.alternative_judge,
		info.draw_mean_instead_of_wifescore,
	)
	.transpose()?;

	let mut embed = serenity::CreateEmbed::default();
	embed
		.color(crate::ETTERNA_COLOR)
		.author(|a| {
			a.name(&score.song.name)
				.url(format!(
					"https://etternaonline.com/song/view/{}",
					score.song.id
				))
				.icon_url(format!(
					"https://etternaonline.com/img/flags/{}.png",
					score.user.country_code.as_deref().unwrap_or("")
				))
		})
		// .thumbnail(format!("https://etternaonline.com/avatars/{}", score.user.avatar)) // takes too much space
		.description(description)
		.timestamp(score.datetime.as_str())
		.footer(|f| {
			f.text(format!("Played by {}", &score.user.username,))
				.icon_url(format!(
					"https://etternaonline.com/avatars/{}",
					score.user.avatar
				))
		});

	if let Some(analysis) = &replay_analysis {
		embed
			.attachment(analysis.replay_graph_path)
			.field(
				"Score comparisons",
				generate_score_comparisons_text(&score, analysis, info.alternative_judge),
				false,
			)
			.field(
				"Tap speeds",
				format!(
					"\
Fastest jack over a course of 20 notes: {:.2} NPS
Fastest total NPS over a course of 100 notes: {:.2} NPS",
					analysis.fastest_finger_jackspeed, analysis.fastest_nps,
				),
				false,
			)
			.field(
				"Combos",
				format!(
					"\
Longest combo: {}
Longest perfect combo: {}
Longest marvelous combo: {}
Longest 100% combo: {}
",
					analysis.longest_combo,
					analysis.longest_perf_combo,
					analysis.longest_marv_combo,
					analysis.longest_100_combo,
				),
				false,
			);
	}

	poise::send_reply(ctx, |f: &mut poise::CreateReply<'_>| {
		f.embed(|e| {
			*e = embed;
			e
		});
		if let Some(analysis) = &replay_analysis {
			f.attachment(analysis.replay_graph_path.into());
		}
		f
	})
	.await?;

	Ok(())
}
