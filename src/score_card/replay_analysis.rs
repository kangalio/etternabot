//! Extracts all data we care about in the score card from a replay

use super::{fun_facts, replay_graph};
use crate::Error;
use etterna::SimpleReplay as _;

pub struct ScoringSystemComparison {
	pub wife2_score: etterna::Wifescore,
	pub wife3_score: etterna::Wifescore,
	pub wife3_score_zero_mean: etterna::Wifescore,
}

pub struct ReplayAnalysis {
	pub replay_graph_path: &'static str,
	pub scoring_system_comparison_j4: ScoringSystemComparison,
	pub scoring_system_comparison_alternative: Option<ScoringSystemComparison>,
	pub fastest_finger_jackspeed: f32, // NPS, single finger
	pub fastest_nps: f32,
	pub longest_100_combo: u32,
	pub longest_marv_combo: u32,
	pub longest_perf_combo: u32,
	pub longest_combo: u32,
	pub mean_offset: f32,
	pub fun_facts: Vec<String>,
}

fn fastest_nps(replay: &etternaonline_api::Replay) -> Option<f32> {
	let note_and_hit_seconds = replay.split_into_notes_and_hits()?;
	let unsorted_hit_seconds = note_and_hit_seconds.hit_seconds;

	let mut sorted_hit_seconds = unsorted_hit_seconds;
	sorted_hit_seconds.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
	let sorted_hit_seconds = sorted_hit_seconds;
	let fastest_nps = etterna::find_fastest_note_subset(&sorted_hit_seconds, 100, 100).speed;

	Some(fastest_nps)
}

fn max_finger_nps(replay: &etternaonline_api::Replay) -> Option<f32> {
	// in the following, DONT scale find_fastest_note_subset results by rate - I only needed
	// to do that for etterna-graph where the note seconds where unscaled. EO's note seconds
	// _are_ scaled though.

	let lanes = replay.split_into_lanes()?;
	let mut max_finger_nps = 0.0;
	for lane in &lanes {
		let mut hit_seconds = lane.hit_seconds.clone();
		// required because EO is jank
		hit_seconds.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());

		let this_fingers_max_nps = etterna::find_fastest_note_subset(&hit_seconds, 20, 20).speed;

		if this_fingers_max_nps > max_finger_nps {
			max_finger_nps = this_fingers_max_nps;
		}
	}

	Some(max_finger_nps)
}

fn adjust_offset(replay: &etternaonline_api::Replay) -> (f32, etternaonline_api::Replay) {
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

	(mean_offset, replay_zero_mean)
}

fn make_scoring_system_comparison(
	score: &etternaonline_api::v1::ScoreData,
	replay: &etternaonline_api::Replay,
	replay_zero_mean: &etternaonline_api::Replay,
	judge: &etterna::Judge,
) -> Option<ScoringSystemComparison> {
	Some(ScoringSystemComparison {
		wife2_score: etternaonline_api::rescore::<etterna::NaiveScorer, etterna::Wife2>(
			replay,
			score.judgements.hit_mines,
			score.judgements.let_go_holds + score.judgements.missed_holds,
			judge,
		)?,
		wife3_score: etternaonline_api::rescore::<etterna::NaiveScorer, etterna::Wife3>(
			replay,
			score.judgements.hit_mines,
			score.judgements.let_go_holds + score.judgements.missed_holds,
			judge,
		)?,
		wife3_score_zero_mean: etternaonline_api::rescore::<etterna::NaiveScorer, etterna::Wife3>(
			replay_zero_mean,
			score.judgements.hit_mines,
			score.judgements.let_go_holds + score.judgements.missed_holds,
			judge,
		)?,
	})
}

pub fn do_replay_analysis(
	score: &etternaonline_api::v1::ScoreData,
	alternative_judge: Option<&etterna::Judge>,
) -> Option<Result<ReplayAnalysis, Error>> {
	let replay = score.replay.as_ref()?;

	let r = replay_graph::generate_replay_graph(replay, "replay_graph.png").transpose()?;
	if let Err(e) = r {
		return Some(Err(e.into()));
	}

	let (mean_offset, replay_zero_mean) = adjust_offset(replay);

	Some(Ok(ReplayAnalysis {
		replay_graph_path: "replay_graph.png",
		scoring_system_comparison_j4: make_scoring_system_comparison(
			score,
			replay,
			&replay_zero_mean,
			etterna::J4,
		)?,
		scoring_system_comparison_alternative: match alternative_judge {
			Some(alternative_judge) => Some(make_scoring_system_comparison(
				score,
				replay,
				&replay_zero_mean,
				alternative_judge,
			)?),
			None => None,
		},
		fastest_finger_jackspeed: max_finger_nps(replay)?,
		fastest_nps: fastest_nps(replay)?,
		longest_100_combo: replay.longest_combo(|hit| hit.is_within_window(0.005)),
		longest_marv_combo: replay
			.longest_combo(|hit| hit.is_within_window(etterna::J4.marvelous_window)),
		longest_perf_combo: replay
			.longest_combo(|hit| hit.is_within_window(etterna::J4.perfect_window)),
		longest_combo: replay.longest_combo(|hit| hit.is_within_window(etterna::J4.great_window)),
		mean_offset,
		fun_facts: fun_facts::make_fun_facts(replay),
	}))
}
