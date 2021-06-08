/// Represents a note pattern without snap changes.
#[derive(Debug, Default)]
pub struct Pattern {
	pub rows: Vec<Row>,
}

#[derive(Debug, Default)]
pub struct Row {
	pub notes: Vec<(Lane, NoteType)>,
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
			Lane::Right => {
				if keymode == 3 {
					2
				} else {
					3
				}
			} // in 3k it goes left-down-right
		}
	}
}

#[derive(PartialEq, Eq, Clone, Debug, Hash, Copy)]
pub enum NoteType {
	Tap,
	Mine,
	Hold { length: u32 },
}
