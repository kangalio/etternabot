/// Represents a simple note pattern without any holds or mines or snap changes.
#[derive(Debug, Default)]
pub struct Pattern {
	/// Each row is a vector of lane numbers. For example a plain jumptrill would be
	/// `vec![vec![0, 1], vec![2, 3], vec![0, 1], vec![2, 3]...]`
	pub rows: Vec<Row>,
}

#[derive(Debug, Default)]
pub struct Row {
	pub notes: Vec<(Lane, NoteType)>,
}

// impl PartialEq for SimplePattern {
// 	fn eq(&self, other: &Self) -> bool {
// 		/// Whether the two slices have the same elements in them, no matter order or duplicates
// 		fn is_same_set<T: PartialEq>(a: &[T], b: &[T]) -> bool {
// 			a.iter().all(|a_elem| b.contains(a_elem)) && b.iter().all(|b_elem| a.contains(b_elem))
// 		}

// 		if self.rows.len() != other.rows.len() {
// 			return false;
// 		}

// 		self.rows
// 			.iter()
// 			.zip(&other.rows)
// 			.all(|(row_a, row_b)| is_same_set(row_a, row_b))
// 	}
// }

// impl Eq for SimplePattern {}

#[derive(PartialEq, Eq, Copy, Clone, Debug, Hash)]
pub enum Lane {
	Index(u32),
	Left,
	Down,
	Up,
	Right,
	Empty,
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
			Lane::Empty => 0, // STUB
		}
	}
}

#[derive(PartialEq, Eq, Clone, Debug, Hash, Copy)]
pub enum NoteType {
	Tap,
	Mine,
	Hold { length: u32 },
}
