use super::{structures::*, Error};

// Pops off the first full character as a substring. This will not panic on
// multi-byte UTF-8 characters.
fn pop_first_char<'a>(string: &mut &'a str) -> Option<&'a str> {
	let (substring, rest) = string.split_at(string.chars().next()?.len_utf8());
	*string = rest;
	Some(substring)
}

#[derive(PartialEq, Eq, Clone, Debug, Hash)]
enum NoteIdentifier {
	Note(Lane, NoteType),
	Empty,
	Invalid,
	ControlCharacter,
}

struct State {
	selected_note_type: NoteType,
}

fn parse_note_identifier(note: &str, state: &mut State) -> NoteIdentifier {
	if let Ok(lane) = note.parse::<u32>() {
		match lane.checked_sub(1) {
			Some(lane) => NoteIdentifier::Note(Lane::Index(lane), state.selected_note_type),
			None => NoteIdentifier::Empty, // 0 means empty row
		}
	} else {
		match note.to_lowercase().as_str() {
			"l" => NoteIdentifier::Note(Lane::Left, state.selected_note_type),
			"d" => NoteIdentifier::Note(Lane::Down, state.selected_note_type),
			"u" => NoteIdentifier::Note(Lane::Up, state.selected_note_type),
			"r" => NoteIdentifier::Note(Lane::Right, state.selected_note_type),
			"" => NoteIdentifier::Empty,
			"m" => {
				state.selected_note_type = NoteType::Mine;
				NoteIdentifier::ControlCharacter
			}
			// other => Err(Error::UnrecognizedNote(other.to_owned())),
			_other => NoteIdentifier::Invalid,
		}
	}
}

/// Will panic if string is too short
fn parse_single_note(pattern: &mut &str, state: &mut State) -> Result<NoteIdentifier, Error> {
	let note;

	if pattern.starts_with('(') {
		let closing_paran = pattern.find(')').ok_or(Error::UnclosedParanthesis)?;

		note = parse_note_identifier(&pattern[1..closing_paran], state);

		*pattern = &pattern[closing_paran + 1..];
	} else {
		// UNWRAP: documented panic behavior
		note = parse_note_identifier(pop_first_char(pattern).unwrap(), state);
	}

	Ok(note)
}

// Will panic if string is too short
// If None is returned, an invalid or control character was popped
fn parse_row(
	pattern: &mut &str,
	state: &mut State,
) -> Result<Option<Vec<(Lane, NoteType)>>, Error> {
	let row = if pattern.starts_with('[') {
		let closing_bracket = pattern.find(']').ok_or(Error::UnclosedBracket)?;

		let mut bracket_contents = &pattern[1..closing_bracket];
		let mut row = Vec::new();
		while !bracket_contents.is_empty() {
			match parse_single_note(&mut bracket_contents, state)? {
				NoteIdentifier::Note(lane, note_type) => row.push((lane, note_type)),
				NoteIdentifier::Empty
				| NoteIdentifier::Invalid
				| NoteIdentifier::ControlCharacter => {}
			}
		}

		*pattern = &pattern[closing_bracket + 1..];

		Some(row)
	} else {
		match parse_single_note(pattern, state)? {
			NoteIdentifier::Note(lane, note_type) => Some(vec![(lane, note_type)]),
			NoteIdentifier::Empty => return Ok(Some(vec![])),
			NoteIdentifier::Invalid => return Ok(None),
			NoteIdentifier::ControlCharacter => return Ok(None),
		}
	};
	state.selected_note_type = NoteType::Tap;
	Ok(row)
}

pub fn parse_pattern(pattern: &str) -> Result<SimplePattern, Error> {
	// remove all whitespace
	let pattern = pattern.split_whitespace().collect::<String>();
	let mut pattern = pattern.as_str();

	let mut state = State {
		selected_note_type: NoteType::Tap,
	};

	let mut rows = Vec::with_capacity(pattern.len() / 2); // rough estimate
	while !pattern.is_empty() {
		if let Some(row) = parse_row(&mut pattern, &mut state)? {
			rows.push(row);
		}
	}

	Ok(SimplePattern { rows })
}
