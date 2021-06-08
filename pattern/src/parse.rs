use super::structures;

/// GENERIC PARSING UTILITES

/// Also handles missing end delimiters gracefully
fn pop_delimited<'a>(t: &'a str, start: &str, end: &str) -> Option<(&'a str, &'a str)> {
	let t = t.strip_prefix(start)?;
	let (inside, rest) = match (t.find(start), t.find(end)) {
		// Like `(12(34)`
		(Some(start_i), Some(end_i)) if start_i < end_i => (&t[..start_i], &t[start_i..]),
		// Like `(12)34` or `(12)(34)`
		(_, Some(end_i)) => (&t[..end_i], &t[(end_i + end.len())..]),
		// Like `(12(34`
		(Some(start_i), None) => (&t[..start_i], &t[start_i..]),
		// Like `(12`
		(None, None) => (t, ""),
	};

	Some((rest, inside))
}

/// Pop first character as &str
fn pop_char_str(t: &str) -> Option<(&str, &str)> {
	let (substring, rest) = t.split_at(t.chars().next()?.len_utf8());
	Some((rest, substring))
}

/// Pop first character as char
fn pop_char(t: &str) -> Option<(&str, char)> {
	let mut chars = t.chars();
	let c = chars.next()?;
	Some((chars.as_str(), c))
}

/// Pop a literal string case-insensitively
fn pop_literal<'a>(t: &'a str, s: &str) -> Option<&'a str> {
	if t.get(0..s.len())?.eq_ignore_ascii_case(s) {
		Some(t.get(s.len()..)?)
	} else {
		None
	}
}

/// INTERNAL PATTERN STRUCTURES

/// Represents a note pattern without snap changes.
#[derive(Debug)]
struct Row {
	notes: Vec<(Lane, NoteType)>,
	// this field is a difference in this internal representation
	move_draw_cursor_to_end_of_holds: bool,
}

#[derive(Debug)]
enum Lane {
	Index(u32),
	Left,
	Down,
	Up,
	Right,
	// this variant is a difference in this internal representation
	Empty,
}

use structures::NoteType; // we can use that struct internally as is no modifications needed

/// PARSING CODE

/// Pop a number, either single digit `8...` or multidigit `(16)...`
fn parse_number(t: &str) -> Option<(&str, Option<u32>)> {
	let (t, number) = pop_delimited(t, "(", ")").or_else(|| pop_char_str(t))?;
	Some((t, number.parse().ok()))
}

/// Option<Lane> is None for invalid char or empty ("0")
fn parse_tap(t: &str) -> Option<(&str, Option<Lane>)> {
	if let Some((t, num)) = parse_number(t) {
		let lane = num.map(|num| num.checked_sub(1).map_or(Lane::Empty, Lane::Index));
		Some((t, lane))
	} else {
		let (t, char_) = pop_char(t)?;
		let lane = match char_.to_ascii_lowercase() {
			'l' => Some(Lane::Left),
			'd' => Some(Lane::Down),
			'u' => Some(Lane::Up),
			'r' => Some(Lane::Right),
			_ => None,
		};
		Some((t, lane))
	}
}

fn parse_note(mut t: &str) -> Option<(&str, Option<(Lane, NoteType)>)> {
	let mut note_type = NoteType::Tap;

	// If prefixed with 'm', change to mine
	if let Some(new_t) = pop_literal(t, "m") {
		t = new_t;
		note_type = NoteType::Mine;
	}

	let (mut t, lane) = parse_tap(t)?;

	// If postfixed by `x<number>`, change to hold
	if let Some((new_t, Some(length))) = pop_literal(t, "x").and_then(parse_number) {
		t = new_t;
		note_type = NoteType::Hold { length };
	}

	let note = lane.map(|lane| (lane, note_type));
	Some((t, note))
}

fn pop_until_none<'a, T: 'a>(
	mut t: &'a str,
	f: fn(&str) -> Option<(&str, T)>,
) -> impl Iterator<Item = T> + 'a {
	std::iter::from_fn(move || {
		let (new_t, data) = f(t)?;
		t = new_t;
		Some(data)
	})
}

fn parse_row(t: &str) -> Option<(&str, Option<Row>)> {
	if let Some((mut t, in_brackets)) = pop_delimited(t, "[", "]") {
		let mut move_draw_cursor_to_end_of_holds = false;

		let mut notes = pop_until_none(in_brackets, parse_note)
			.flatten()
			.collect::<Vec<_>>();

		// If postfixed by `x<number>`, change entire row to hold
		if let Some((new_t, Some(length))) = pop_literal(t, "x").and_then(parse_number) {
			t = new_t;

			if notes.is_empty() {
				// If you write `[]x10` it should have the same effect as `0x10`
				notes.push((Lane::Empty, NoteType::Hold { length }));
				move_draw_cursor_to_end_of_holds = true;
			} else {
				for note in &mut notes {
					note.1 = NoteType::Hold { length };
				}
			}
		}

		let row = Row {
			notes,
			move_draw_cursor_to_end_of_holds,
		};
		Some((t, Some(row)))
	} else {
		let (t, note) = parse_note(t)?;
		let row = note.map(|note| Row {
			notes: vec![note],
			move_draw_cursor_to_end_of_holds: true,
		});
		Some((t, row))
	}
}

pub fn parse_pattern(pattern: &str) -> structures::Pattern {
	// Parse the pattern into internal representation, then convert to more practical representation
	let mut rows = Vec::new();
	for row in pop_until_none(pattern, parse_row).flatten() {
		let mut extra_empty_rows_to_insert = 0;
		if row.move_draw_cursor_to_end_of_holds {
			let max_hold_length = row
				.notes
				.iter()
				.filter_map(|(_, note_type)| match note_type {
					NoteType::Hold { length } => Some(length),
					_ => None,
				})
				.max();
			if let Some(max_hold_length) = max_hold_length {
				extra_empty_rows_to_insert = max_hold_length.saturating_sub(1);
			}
		}

		let notes = row
			.notes
			.into_iter()
			.filter_map(|(lane, note_type)| {
				let lane = match lane {
					Lane::Index(num) => structures::Lane::Index(num),
					Lane::Left => structures::Lane::Left,
					Lane::Down => structures::Lane::Down,
					Lane::Up => structures::Lane::Up,
					Lane::Right => structures::Lane::Right,
					Lane::Empty => return None,
				};
				Some((lane, note_type))
			})
			.collect::<Vec<_>>();
		rows.push(structures::Row { notes });

		for _ in 0..extra_empty_rows_to_insert {
			rows.push(structures::Row { notes: vec![] });
		}
	}

	structures::Pattern { rows }
}
