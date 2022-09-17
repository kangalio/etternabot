fn try_undo_pig_latin(s: &str) -> Option<String> {
	if let Some(s) = s.strip_suffix("yay") {
		return Some(s.to_string());
	}

	if let Some(s) = s.strip_suffix("ay") {
		if let Some(last_char) = s.chars().last() {
			if "bcdfghjklmnpqrstvwxyz".contains(last_char) {
				return Some(last_char.to_string() + &s[..s.len() - 1]);
			}
		}
	}

	None
}

fn make_pig_latin(s: &str) -> String {
	let first_char = match s.chars().next() {
		Some(x) => x,
		None => return String::new(),
	};

	if "aeiou".contains(first_char) {
		s.to_string() + "yay"
	} else {
		let consonant_cluster_end = s.find(|c| "aeiou".contains(c)).unwrap_or(s.len());
		s[consonant_cluster_end..].to_string() + &s[..consonant_cluster_end] + "ay"
	}
}

/// Detects either reverse order or pig latin and returns the identified command, the detransformed
/// string and the detected transformation as a function
///
/// Must be called with a lowercase string
pub fn detect<C>(
	s: &str,
	find_command: impl Fn(&str) -> Option<C>,
) -> Option<(C, String, fn(&str) -> String)> {
	let reversed = s.chars().rev().collect::<String>();
	if let Some(command) = find_command(&reversed) {
		return Some((command, reversed, |s| s.chars().rev().collect()));
	}

	if let Some(s) = try_undo_pig_latin(s) {
		if let Some(command) = find_command(&s) {
			return Some((command, s, make_pig_latin));
		}
	}

	None
}
