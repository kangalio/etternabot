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
	pub rows: Vec<Vec<Lane>>,
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

#[derive(PartialEq, Eq, Clone, Debug, Hash)]
enum NoteIdentifier {
    Lane(Lane),
    Empty,
    Invalid,
}

fn parse_note_identifier(note: &str) -> Result<NoteIdentifier, PatternParseError> {
    if let Ok(lane) = note.parse::<u32>() {
        if lane == 0 {
            Ok(NoteIdentifier::Empty)
        } else {
            // Must have checked that lane isn't zero! to prevent underflow
            Ok(NoteIdentifier::Lane(Lane::Index(lane - 1)))
        }
    } else {
        match note.to_lowercase().as_str() {
            "l" => Ok(NoteIdentifier::Lane(Lane::Left)),
            "d" => Ok(NoteIdentifier::Lane(Lane::Down)),
            "u" => Ok(NoteIdentifier::Lane(Lane::Up)),
            "r" => Ok(NoteIdentifier::Lane(Lane::Right)),
            "" => Ok(NoteIdentifier::Empty),
            // other => Err(PatternParseError::UnrecognizedNote(other.to_owned())),
            _other => Ok(NoteIdentifier::Invalid),
        }
    }
}

// Will panic if string is too short
fn parse_single_note(pattern: &mut &str) -> Result<NoteIdentifier, PatternParseError> {
    let note;

    if pattern.starts_with('(') {
        let closing_paran = pattern.find(')').ok_or(PatternParseError::UnclosedParanthesis)?;
        
        note = parse_note_identifier(&pattern[1..closing_paran])?;
        
        *pattern = &pattern[closing_paran+1..];
    } else {
        note = parse_note_identifier(pop_first_char(pattern).unwrap())?;
    }
    
    Ok(note)
}

// Will panic if string is too short
// If None is returned, an invalid character was popped
fn parse_row(pattern: &mut &str) -> Result<Option<Vec<Lane>>, PatternParseError> {
    if pattern.starts_with('[') {
        let closing_bracket = pattern.find(']').ok_or(PatternParseError::UnclosedBracket)?;
        
        let mut bracket_contents = &pattern[1..closing_bracket];
        let mut row = Vec::new();
        while !bracket_contents.is_empty() {
            match parse_single_note(&mut bracket_contents)? {
                NoteIdentifier::Lane(lane) => row.push(lane),
                NoteIdentifier::Empty | NoteIdentifier::Invalid => {},
            }
        }
        
        *pattern = &pattern[closing_bracket+1..];
        
        Ok(Some(row))
    } else {
        match parse_single_note(pattern)? {
            NoteIdentifier::Lane(lane) => Ok(Some(vec![lane])),
            NoteIdentifier::Empty => Ok(Some(vec![])),
            NoteIdentifier::Invalid => Ok(None),
        }
    }
}

pub fn parse_pattern(pattern: &str) -> Result<SimplePattern, PatternParseError> {
    // remove all whitespace
    let pattern = pattern.split_whitespace().collect::<String>();
    let mut pattern = pattern.as_str();

    let mut rows = Vec::with_capacity(pattern.len() / 2); // rough estimate
    while !pattern.is_empty() {
        if let Some(row) = parse_row(&mut pattern)? {
            rows.push(row);
        }
    }
    
    Ok(SimplePattern { rows })
}

#[cfg(test)]
mod tests {
	use super::*;

	// I think I'm making useless tests again. I probably don't even need tests for these miniscule
	// functions, in fact they're probably gonna change so often that these tests are gonna be out-
	// dated all the time..... eh whatever, the code is written, too late now.

	#[test]
	fn test_pattern_equality() {
		assert_eq!(
			SimplePattern { rows: vec![vec![Lane::Index(0), Lane::Index(1), Lane::Index(2)]] },
			SimplePattern { rows: vec![vec![Lane::Index(2), Lane::Index(1), Lane::Index(0)]] },
		);
		assert_eq!(
			SimplePattern { rows: vec![vec![Lane::Index(0), Lane::Index(1), Lane::Index(2), Lane::Index(2)]] },
			SimplePattern { rows: vec![vec![Lane::Index(2), Lane::Index(1), Lane::Index(0)]] },
		);
		assert_ne!(
			SimplePattern { rows: vec![vec![Lane::Index(0), Lane::Index(1), Lane::Index(2), Lane::Index(3)]] },
			SimplePattern { rows: vec![vec![Lane::Index(0), Lane::Index(1), Lane::Index(2), Lane::Index(2)]] },
		);
	}
}