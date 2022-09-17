/// Detects if this string, reversed, matches any commands, and returns the command and reversed
/// string
pub fn detect<C>(s: &str, find_command: impl Fn(&str) -> Option<C>) -> Option<(C, String)> {
	let reversed = s.chars().rev().collect::<String>();
	if let Some(command) = find_command(&reversed) {
		return Some((command, reversed));
	}
	None
}
