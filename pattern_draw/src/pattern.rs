use thiserror::Error;

fn is_equal_no_order_no_duplicates<T: PartialEq>(a: &[T], b: &[T]) -> bool {
	a.iter().all(|a_elem| b.contains(a_elem))
	&& b.iter().all(|b_elem| a.contains(b_elem))
}

#[derive(Debug, Error)]
pub enum PatternParseError {
	#[error("Missing closing bracket")]
	UnclosedBracket,
	#[error("Missing closing paranthesis")]
	UnclosedParanthesis,
	#[error("Unrecognized note \"{0}\". Only numbers and L/D/U/R can be used as lanes")]
    UnrecognizedNote(String),
}

/// Represents a simple note pattern without any holds or mines or snap changes.
#[derive(Debug, Default)]
pub struct SimplePattern {
	/// Each row is a vector of lane numbers. For example a plain jumptrill would be
	/// `vec![vec![0, 1], vec![2, 3], vec![0, 1], vec![2, 3]...]`
	pub rows: Vec<Vec<(Lane, NoteType)>>,
}

impl PartialEq for SimplePattern {
    fn eq(&self, other: &Self) -> bool {
		if self.rows.len() != other.rows.len() { return false; }
		
		self.rows.iter().zip(&other.rows)
			.all(|(row_a, row_b)| is_equal_no_order_no_duplicates(row_a, row_b))
    }
}

impl Eq for SimplePattern {}

// Pops off the first full character as a substring. This will not panic on
// multi-byte UTF-8 characters.
fn pop_first_char<'a>(string: &mut &'a str) -> Option<&'a str> {
    let (substring, rest) = string.split_at(string.chars().next()?.len_utf8());
    *string = rest;
    Some(substring)
}

#[derive(PartialEq, Eq, Copy, Clone, Debug, Hash)]
pub enum Lane {
    Index(u32),
    Left,
    Down,
    Up,
    Right,
}

impl Lane {
    pub fn column_number_with_keymode(&self, keymode: u32) -> u32 {
        match *self {
            Lane::Index(lane) => lane,
            Lane::Left => 0,
            Lane::Down => 1,
            Lane::Up => 2,
            Lane::Right => if keymode == 3 { 2 } else { 3 }, // in 3k it goes left-down-right
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Hash, Copy)]
pub enum NoteType {
    Tap,
    Mine,
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

fn parse_note_identifier(note: &str, state: &mut State) -> Result<NoteIdentifier, PatternParseError> {
    if let Ok(lane) = note.parse::<u32>() {
        if lane == 0 {
            Ok(NoteIdentifier::Empty)
        } else {
            // Must have checked that lane isn't zero! to prevent underflow
            Ok(NoteIdentifier::Note(Lane::Index(lane - 1), state.selected_note_type))
        }
    } else {
        match note.to_lowercase().as_str() {
            "l" => Ok(NoteIdentifier::Note(Lane::Left, state.selected_note_type)),
            "d" => Ok(NoteIdentifier::Note(Lane::Down, state.selected_note_type)),
            "u" => Ok(NoteIdentifier::Note(Lane::Up, state.selected_note_type)),
            "r" => Ok(NoteIdentifier::Note(Lane::Right, state.selected_note_type)),
            "" => Ok(NoteIdentifier::Empty),
            "m" => {
                state.selected_note_type = NoteType::Mine;
                Ok(NoteIdentifier::ControlCharacter)
            },
            // other => Err(PatternParseError::UnrecognizedNote(other.to_owned())),
            _other => Ok(NoteIdentifier::Invalid),
        }
    }
}

/// Will panic if string is too short
fn parse_single_note(pattern: &mut &str, state: &mut State) -> Result<NoteIdentifier, PatternParseError> {
    let note;

    if pattern.starts_with('(') {
        let closing_paran = pattern.find(')').ok_or(PatternParseError::UnclosedParanthesis)?;
        
        note = parse_note_identifier(&pattern[1..closing_paran], state)?;
        
        *pattern = &pattern[closing_paran+1..];
    } else {
        // UNWRAP: documented panic behavior
        note = parse_note_identifier(pop_first_char(pattern).unwrap(), state)?;
    }
    
    Ok(note)
}

// Will panic if string is too short
// If None is returned, an invalid or control character was popped
fn parse_row(pattern: &mut &str, state: &mut State) -> Result<Option<Vec<(Lane, NoteType)>>, PatternParseError> {
    let row = if pattern.starts_with('[') {
        let closing_bracket = pattern.find(']').ok_or(PatternParseError::UnclosedBracket)?;
        
        let mut bracket_contents = &pattern[1..closing_bracket];
        let mut row = Vec::new();
        while !bracket_contents.is_empty() {
            match parse_single_note(&mut bracket_contents, state)? {
                NoteIdentifier::Note(lane, note_type) => row.push((lane, note_type)),
                NoteIdentifier::Empty | NoteIdentifier::Invalid | NoteIdentifier::ControlCharacter => {},
            }
        }
        
        *pattern = &pattern[closing_bracket+1..];
        
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

pub fn parse_pattern(pattern: &str) -> Result<SimplePattern, PatternParseError> {
    // remove all whitespace
    let pattern = pattern.split_whitespace().collect::<String>();
    let mut pattern = pattern.as_str();

    let mut state = State { selected_note_type: NoteType::Tap };

    let mut rows = Vec::with_capacity(pattern.len() / 2); // rough estimate
    while !pattern.is_empty() {
        if let Some(row) = parse_row(&mut pattern, &mut state)? {
            rows.push(row);
        }
    }
    
    Ok(SimplePattern { rows })
}