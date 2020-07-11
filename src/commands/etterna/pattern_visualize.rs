#![allow(clippy::collapsible_if)]

use image::{GenericImageView, GenericImage, RgbaImage};


/// An ad-hoc error type that fits any string literal
#[derive(Debug)]
pub struct StringError(&'static str);
impl std::fmt::Display for StringError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.0.fmt(f)
  }
}
impl std::error::Error for StringError {}

/// Read the given noteskin image path and split it into multiple note images, each of size 64x64
fn read_noteskin(noteskin_path: &str) -> Result<Vec<RgbaImage>, Box<dyn std::error::Error>> {
  let mut img = image::open(noteskin_path)?;
  assert_eq!(img.width(), 64);

  let mut notes = Vec::new();
  for y in (0..img.height()).step_by(64) {
    notes.push(img.crop(0, y, 64, 64).into_rgba());
  }

  Ok(notes)
}

struct Pattern {
  /// Each row is a vector of lane numbers. For example a plain jumptrill would be
  /// `vec![vec![0, 1], vec![2, 3], vec![0, 1], vec![2, 3]...]`
  pub rows: Vec<Vec<u32>>,
}

/// Parameter `note_imgs`: a slice of 64x64 images, in the following order: 4ths, 8ths, 12ths,
/// 16ths, 24ths, 32nds, 48ths, 64ths, 192nds
fn render_pattern(note_imgs: &[RgbaImage], pattern: &Pattern) -> Result<RgbaImage, Box<dyn std::error::Error>> {
  // Determines the keymode (e.g. 4k/5k/6k/...) by finding the rightmost mentioned lane and adding 1
  let keymode = 1 + *pattern.rows.iter().flatten().max()
      .ok_or(StringError("Given pattern is empty"))?;

  // Create an empty image buffer, big enough to fit all the lanes and arrows
  let mut buffer = image::ImageBuffer::new(64 * keymode, 64 * pattern.rows.len() as u32);

  for (i, row) in pattern.rows.iter().enumerate() {
    // Select a note image in the order of 4th-16th-8th-16th-(cycle repeats)
    let note_img = &note_imgs[[0, 3, 1, 3][i % 4]];

    let y = i * 64;

    for lane in row {
      buffer.copy_from(note_img, 64 * lane, y as u32)
          .expect("Note image is too large");
    }
  }
  
  Ok(buffer)
}

/// Returns the width in bytes of the first character in the string
fn first_char_width(string: &str) -> usize {
  for i in 1.. {
    if string.is_char_boundary(i) {
      return i;
    }
  }
  unreachable!();
}

fn parse_pattern(mut string: &str) -> Result<Pattern, Box<dyn std::error::Error>> {
  let mut rows = Vec::new();

  // this parser works by 'popping' characters off the start of the string until the string is empty

  while !string.is_empty() {
    // if the next char is a '[', find the matching ']', read all numbers inbetween, put them into a
    // vector, and finally add that vector to the `rows`
    // if the next char is _not_ a '[' and it's a valid number, push a new row with the an arrow in
    // the lane specified by the number
    if string.starts_with('[') {
      let end = string.find(']')
          .ok_or(StringError("Unterminated ["))?;
      
      let mut row = Vec::new();
      for c in string[1..end].bytes() {
        if c >= b'1' && c <= b'9' {
          row.push((c - b'1') as u32);
        }
      }
  
      rows.push(row);
  
      string = &string[end+1..];
    } else {
      let c = string.as_bytes()[0];
      if c >= b'1' && c <= b'9' {
        rows.push(vec![(c - b'1') as u32]);
      }

      string = &string[first_char_width(string)..];
    }
  }

  Ok(Pattern { rows })
}

/// Read noteskin from `noteskin_path`, read the pattern from `pattern_str` and write the generated
/// image into `output_path`
pub fn generate(
  noteskin_path: &str,
  output_path: &str,
  pattern_str: &str
) -> Result<(), Box<dyn std::error::Error>> {

  let note_imgs = read_noteskin(noteskin_path)?;
  let pattern = parse_pattern(pattern_str)?;
  let buffer = render_pattern(&note_imgs, &pattern)?;
  
  buffer.save(output_path)?;

  Ok(())
}