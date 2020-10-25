#[derive(Copy, Clone, Hash, PartialEq, Eq)]
pub struct FractionalSnap {
	snap_number: std::num::NonZeroU32,
}

impl FractionalSnap {
	pub fn from_snap_number(snap_number: u32) -> Option<Self> {
		Some(Self { snap_number: std::num::NonZeroU32::new(snap_number)? })
	}

	/// The snap number will always be above 0.
	pub fn snap_number(&self) -> u32 {
		self.snap_number.get()
	}

	pub fn iter_192nd_intervals(&self) -> Iter192ndIntervals {
		Iter192ndIntervals {
			exact_192nd_interval: 192.0 / self.snap_number.get() as f32,
			carry: 0.0,
		}
	}
}

impl From<etterna::Snap> for FractionalSnap {
	fn from(snap: etterna::Snap) -> Self {
		match snap {
			etterna::Snap::_4th => Self::from_snap_number(4).unwrap(),
			etterna::Snap::_8th => Self::from_snap_number(8).unwrap(),
			etterna::Snap::_12th => Self::from_snap_number(12).unwrap(),
			etterna::Snap::_16th => Self::from_snap_number(16).unwrap(),
			etterna::Snap::_24th => Self::from_snap_number(24).unwrap(),
			etterna::Snap::_32th => Self::from_snap_number(32).unwrap(),
			etterna::Snap::_48th => Self::from_snap_number(48).unwrap(),
			etterna::Snap::_64th => Self::from_snap_number(64).unwrap(),
			etterna::Snap::_192th => Self::from_snap_number(192).unwrap(),
		}
	}
}

pub struct Iter192ndIntervals {
	exact_192nd_interval: f32,
	carry: f32,
}

impl Iter192ndIntervals {
	pub fn next_interval(&mut self) -> u32 {
		let interval = self.exact_192nd_interval + self.carry;
		self.carry = interval.fract();
		interval.floor() as u32
	}
}

impl Iterator for Iter192ndIntervals {
	type Item = u32;

	fn next(&mut self) -> Option<Self::Item> {
		Some(Iter192ndIntervals::next_interval(self))
	}
}