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
	pub rows: Vec<Vec<u32>>,
}

impl SimplePattern {
	/// Guesses the keymode (e.g. 4k/5k/6k/...) by adding 1 to the rightmost lane. The number is
	/// clamped to a minimum of 4k - there is no such thing as 3k, 2k, 1k.
	/// 
	/// Returns None if the pattern is empty.
	///
	/// Note that this function returns only a _guess_. Nobody knows if \[12\]\[34\] was intended as
	/// a 4k pattern, or a 5k, 6k, 7k...
	/// 
	/// ```rust
	/// # use etterna_base::Pattern;
	/// # fn main() -> Result<(), Box<dyn std::error::PatternParseError>> {
	/// assert_eq!(Pattern::parse_taps("1234").keymode(), Some(4));
	/// assert_eq!(Pattern::parse_taps("123").keymode(), Some(4));
	/// assert_eq!(Pattern::parse_taps("9").keymode(), Some(9));
	/// assert_eq!(Pattern::parse_taps("").keymode(), None);
	/// # Ok(()) }
	/// ```
	pub fn keymode_guess(&self) -> Option<u32> {
		let keymode = 1 + self.rows.iter().flatten().max()?;

		// clamp to a minimum of 4 because even if the pattern does not use all four columns, it's
		// still at least 4k
		let keymode = keymode.max(4);

		Some(keymode)
	}
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

// Returns None if character signifies an empty space
fn parse_note_identifier(note: &str) -> Result<Option<u32>, PatternParseError> {
    if let Ok(lane) = note.parse::<u32>() {
        if lane == 0 {
            Ok(None)
        } else {
            // Must have checked that lane isn't zero!
            Ok(Some(lane - 1))
        }
    } else {
        match note.to_lowercase().as_str() {
            "l" => Ok(Some(0)),
            "d" => Ok(Some(1)),
            "u" => Ok(Some(2)),
            "r" => Ok(Some(3)),
            "" => Ok(None),
            other => Err(PatternParseError::UnrecognizedNote(other.to_owned())),
        }
    }
}

// Will panic if string is too short
fn parse_single_note(pattern: &mut &str) -> Result<Option<u32>, PatternParseError> {
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
fn parse_row(pattern: &mut &str) -> Result<Vec<u32>, PatternParseError> {
    if pattern.starts_with('[') {
        let closing_bracket = pattern.find(']').ok_or(PatternParseError::UnclosedBracket)?;
        
        let mut bracket_contents = &pattern[1..closing_bracket];
        let mut row = Vec::new();
        while !bracket_contents.is_empty() {
            if let Some(note) = parse_single_note(&mut bracket_contents)? {
                row.push(note);
            } // else, something like 0 or () was entered
        }
        
        *pattern = &pattern[closing_bracket+1..];
        
        Ok(row)
    } else {
        match parse_single_note(pattern)? {
            Some(note) => Ok(vec![note]),
            None => Ok(vec![]),
        }
    }
}

pub fn parse_pattern(pattern: &str) -> Result<SimplePattern, PatternParseError> {
    // remove all whitespace
    let pattern = pattern.split_whitespace().collect::<String>();
    let mut pattern = pattern.as_str();

    let mut rows = Vec::with_capacity(pattern.len() / 2); // rough estimate
    while !pattern.is_empty() {
        rows.push(parse_row(&mut pattern)?);
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
			SimplePattern { rows: vec![vec![0, 1, 2]] },
			SimplePattern { rows: vec![vec![2, 1, 0]] },
		);
		assert_eq!(
			SimplePattern { rows: vec![vec![0, 1, 2, 2]] },
			SimplePattern { rows: vec![vec![2, 1, 0]] },
		);
		assert_ne!(
			SimplePattern { rows: vec![vec![0, 1, 2, 3]] },
			SimplePattern { rows: vec![vec![0, 1, 2, 2]] },
		);
	}
}