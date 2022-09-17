fn is_scream(s: &str) -> bool {
	s.bytes().all(|b| !b.is_ascii_lowercase())
}

fn make_scream(s: &mut String) {
	s.make_ascii_uppercase()
}

fn is_spongebob(s: &str) -> bool {
	// Filter out tiny strings and "Xx" which is title case not spongebob
	let (first, rest) = match s.as_bytes() {
		[] | [_] => return false,
		[a, b] if a.is_ascii_uppercase() && b.is_ascii_lowercase() => return false,
		[first, rest @ ..] => (first, rest),
	};

	let mut prev_was_uppercase = first.is_ascii_uppercase();
	for c in rest {
		if prev_was_uppercase == c.is_ascii_uppercase() {
			return false;
		}
		prev_was_uppercase = c.is_ascii_uppercase();
	}
	true
}

fn make_spongebob(s: &mut String) {
	let mut uppercase = true;
	*s = s
		.chars()
		.map(|c| {
			uppercase = !uppercase;
			match uppercase {
				true => c.to_ascii_uppercase(),
				false => c.to_ascii_lowercase(),
			}
		})
		.collect()
}

fn is_reverse_scream(s: &str) -> bool {
	match s.as_bytes() {
		[first, rest @ ..] if rest.len() > 0 => {
			first.is_ascii_lowercase() && rest.iter().all(|c| c.is_ascii_uppercase())
		}
		_ => false,
	}
}

fn make_reverse_scream(s: &mut String) {
	fn transform_single_word(s: &str) -> String {
		let mut is_first = true;
		s.chars()
			.map(|c| {
				if is_first {
					is_first = false;
					c.to_ascii_lowercase()
				} else {
					c.to_ascii_uppercase()
				}
			})
			.collect()
	}

	let words = s
		.split(|c: char| !c.is_alphabetic())
		.filter(|&s| s != "")
		.map(|word| {
			let index = word.as_ptr() as usize - s.as_ptr() as usize;
			let transformed = transform_single_word(word);
			(index, transformed)
		})
		.collect::<Vec<_>>();

	for (index, transformed) in words {
		s.replace_range(index..index + transformed.len(), &transformed);
	}
}

pub fn detect(template: &str) -> Option<fn(&mut String)> {
	if is_scream(template) {
		Some(make_scream)
	} else if is_spongebob(template) {
		Some(make_spongebob)
	} else if is_reverse_scream(template) {
		Some(make_reverse_scream)
	} else {
		None
	}
}
