use image::GenericImage as _;

mod parse;
pub use parse::*;

mod noteskin;
pub use noteskin::*;

mod fractional_snap;
pub use fractional_snap::*;

mod render;
pub use render::*;

mod structures;
pub use structures::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("Given pattern is empty")]
	EmptyPattern,
	#[error("Error in the image library")]
	ImageError(#[from] image::ImageError),
	#[error("{keymode}k not supported by selected noteskin")]
	NoteskinDoesntSupportKeymode { keymode: usize },
	#[error("Lane {human_readable_lane} is invalid in {keymode}k")]
	InvalidLaneForKeymode {
		human_readable_lane: usize,
		keymode: usize,
	},
	#[error("Noteskin's texture map doesn't contain all required textures")]
	NoteskinTextureMapTooSmall,
	#[error("{count} sprites would need to be rendered for this pattern, which exceeds the limit of {limit}")]
	TooManySprites { count: usize, limit: usize },
	#[error("Rendered pattern would exceed the limit of {max_width}x{max_height}")]
	ImageTooLarge {
		width: usize,
		height: usize,
		max_width: usize,
		max_height: usize,
	},
	#[error("Holds are not supported yet")]
	HoldsAreUnsupported,
}

trait Warn<T>: Sized {
	#[track_caller]
	fn warn(self) -> Option<T>;

	#[track_caller]
	fn warn_or_default(self) -> T
	where
		T: Default,
	{
		self.warn().unwrap_or_default()
	}
}
impl<T, E: std::fmt::Display> Warn<T> for Result<T, E> {
	#[track_caller]
	fn warn(self) -> Option<T> {
		match self {
			Ok(x) => Some(x),
			Err(e) => {
				log::warn!(
					"unexpected error: {}\n{}",
					e,
					std::backtrace::Backtrace::capture()
				);
				None
			}
		}
	}
}
