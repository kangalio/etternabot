use etterna::Wife as _;

/// Returns the wife points for the replay note and the number of notes it should be counted as
/// (0 or 1)
fn replay_note_wife_points(note: &etternaonline_api::ReplayNote) -> (f32, u32) {
	let mut wife_points = 0.0;
	let mut num_notes = 0;

	// I have zero idea what I am doing
	// (more specifically: I have zero idea if and how EO represents mines or holds in replays and
	// whether the way I'm handling them here is remotely correct)
	match note.note_type.unwrap_or(etterna::NoteType::Tap) {
		etterna::NoteType::Tap | etterna::NoteType::HoldHead | etterna::NoteType::Lift => {
			wife_points = etterna::wife3(note.hit, etterna::J4);
			num_notes = 1;
		}
		etterna::NoteType::HoldTail => {
			if note.hit.is_considered_miss(etterna::J4) {
				wife_points = etterna::Wife3::HOLD_DROP_WEIGHT;
			}
		}
		etterna::NoteType::Mine => {
			if let etterna::Hit::Hit { .. } = note.hit {
				wife_points = etterna::Wife3::MINE_HIT_WEIGHT;
			}
		}
		etterna::NoteType::Keysound | etterna::NoteType::Fake => {}
	}

	(wife_points, num_notes)
}

fn calculate_hand_wifescores(replay: &etternaonline_api::Replay) -> (f32, f32) {
	let mut left_wife_points = 0.0;
	let mut left_num_notes = 0;
	let mut right_wife_points = 0.0;
	let mut right_num_notes = 0;

	for note in &replay.notes {
		let (wife_points, num_notes) = match note.lane {
			Some(0 | 1) => (&mut left_wife_points, &mut left_num_notes),
			Some(2 | 3) => (&mut right_wife_points, &mut right_num_notes),
			_ => continue,
		};

		let (note_wife_points, note_num_notes) = replay_note_wife_points(note);
		*wife_points += note_wife_points;
		*num_notes += note_num_notes;
	}

	let left_wifescore = left_wife_points / left_num_notes as f32;
	let right_wifescore = right_wife_points / right_num_notes as f32;

	(left_wifescore, right_wifescore)
}

// TODO: incorpoprate hit mines and dropped holds by distributing them equally on hands
fn make_left_right_hand_difference_fun_fact(
	fun_facts: &mut Vec<String>,
	replay: &etternaonline_api::Replay,
) {
	let (left_wifescore, right_wifescore) = calculate_hand_wifescores(replay);

	let (better_hand, better_hand_name, lower_hand, lower_hand_name) =
		if left_wifescore > right_wifescore {
			(left_wifescore, "left", right_wifescore, "right")
		} else {
			(right_wifescore, "right", left_wifescore, "left")
		};

	// Check if one hand played twice as good as the other
	if (1.0 - lower_hand) / (1.0 - better_hand) >= 2.0 {
		fun_facts.push(format!(
			"Your {} hand played {:.02}% better than your {} hand ({:.02}% vs {:.02}%. Are you {}-handed? ;)",
			better_hand_name,
			(better_hand - lower_hand) * 100.0,
			lower_hand_name,
			(better_hand) * 100.0,
			(lower_hand) * 100.0,
			better_hand_name,
		));
	}
}

/*
fn make_hit_outliers_fun_fact(fun_facts: &mut Vec<String>, replay: &etternaonline_api::Replay) {
	let mut notes = replay
		.notes
		.iter()
		.map(replay_note_wife_points)
		.collect::<Vec<_>>();

	let mut total_wifepoints = 0.0;
	let mut total_num_notes = 0;
	for (wifepoints, num_notes) in &notes {
		total_wifepoints += wifepoints;
		total_num_notes += num_notes;
	}

	#[derive(PartialOrd, PartialEq)]
	struct NoisyFloat(f32);
	// https://github.com/rust-lang/rust-clippy/issues/6219
	#[allow(clippy::derive_ord_xor_partial_ord)]
	impl Ord for NoisyFloat {
		fn cmp(&self, other: &Self) -> std::cmp::Ordering {
			self.0.partial_cmp(&other.0).unwrap()
		}
	}
	impl Eq for NoisyFloat {}

	// Sort descendingly by the wifescore we'd have with the note excluded
	notes.sort_by_cached_key(|(wifepoints, num_notes)| {
		let new_wifescore = (total_wifepoints - wifepoints) / (total_num_notes - num_notes) as f32;
		std::cmp::Reverse(NoisyFloat(new_wifescore))
	});

	// Now, starting with most negatively impactful notes, see how many we need to exclude to get a
	// sudden jump in wifescore
	let old_wifescore = total_wifepoints / total_num_notes as f32;
	for (i, (wifepoints, num_notes)) in notes.iter().take(10).enumerate() {
		total_wifepoints -= wifepoints;
		total_num_notes -= num_notes;
		let new_wifescore = total_wifepoints / total_num_notes as f32;

		// yes i like meth ehh i mean math
		let excluded_note_proportion = (i + 1) as f32 / total_num_notes as f32;
		let multiplier_threshold = 1.0 / (1.0 - excluded_note_proportion).powf(100.0);
		println!(
			"With {:.02}% excluded, the new score needs to be {:.02}x better (is {:.02}x)",
			excluded_note_proportion * 100.0,
			multiplier_threshold,
			(1.0 - old_wifescore) / (1.0 - new_wifescore)
		);
		if (1.0 - old_wifescore) / (1.0 - new_wifescore) >= multiplier_threshold {
			fun_facts.push(format!(
				"Would have been {:.02}% (instead of {:.02}%) without those {} pesky outliers",
				new_wifescore * 100.0,
				old_wifescore * 100.0,
				i + 1,
			));
			break;
		}
	}
}
*/

fn calculate_sd(replay: &etternaonline_api::Replay) -> f32 {
	let mut num_hits = 0;
	let sum_of_squared_deviations = replay
		.notes
		.iter()
		.filter_map(|note| note.hit.deviation())
		.map(|deviation| deviation * deviation)
		.inspect(|_| num_hits += 1)
		.sum::<f32>();

	(sum_of_squared_deviations / num_hits as f32).sqrt()
}

fn make_hit_outliers_fun_fact(
	fun_facts: &mut Vec<String>,
	judgements: &etterna::FullJudgements,
	replay: &etternaonline_api::Replay,
) {
	let deviation_threshold = 5.0 * calculate_sd(replay);
	dbg!(deviation_threshold);

	let original_wifescore = etterna::Wife3::apply(
		replay.notes.iter().map(|note| note.hit),
		judgements.hit_mines,
		judgements.missed_holds + judgements.let_go_holds,
		etterna::J4,
	);

	let filtered_hits = replay.notes.iter().map(|note| note.hit).filter(|hit| {
		let deviation = hit.deviation().unwrap_or(etterna::J4.bad_window).abs();
		deviation < deviation_threshold
	});
	let num_removed_outliers = replay.notes.len() - filtered_hits.clone().count();
	let filtered_wifescore = etterna::Wife3::apply(
		filtered_hits,
		judgements.hit_mines,
		judgements.missed_holds + judgements.let_go_holds,
		etterna::J4,
	);

	let (original_wifescore, filtered_wifescore) = match (original_wifescore, filtered_wifescore) {
		(Some(a), Some(b)) => (a, b),
		_ => return,
	};

	// if (1.0 - old_wifescore) / (1.0 - new_wifescore) >= multiplier_threshold {
	if true {
		fun_facts.push(format!(
			"Would have been {:.02}% (instead of {:.02}%) with {} outliers removed",
			filtered_wifescore.as_percent(),
			original_wifescore.as_percent(),
			num_removed_outliers,
		));
	}
}

pub fn make_fun_facts(
	judgements: &etterna::FullJudgements,
	replay: &etternaonline_api::Replay,
) -> Vec<String> {
	let mut fun_facts = Vec::new();

	make_left_right_hand_difference_fun_fact(&mut fun_facts, replay);
	make_hit_outliers_fun_fact(&mut fun_facts, judgements, replay);

	fun_facts
}
