use super::screen::{Point, Screen};

use noto_sans_mono_bitmap::{get_bitmap, BitmapHeight, FontWeight};

const COLUMNS: usize = 80;
const LINES: usize = 24;

pub struct TextRenderer {
    orig: Point,
}

impl TextRenderer {
    pub fn new(_orig: Point) -> Self {
        TextRenderer {
            orig: Point::default(),
        }
    }

    pub fn draw_char(c: char, screen: &mut Screen<u32>, at: Point) -> Result<(), &'static str> {
        let bitmap =
            get_bitmap(c, FontWeight::Regular, BitmapHeight::Size14).ok_or("Unsupported char")?;
        let mut tmp = [0u8; 4];

        for (row_i, row) in bitmap.bitmap().iter().enumerate() {
            for (col_i, intensity) in row.iter().enumerate() {
                tmp.fill(*intensity);
                let pixel = core::primitive::u32::from_ne_bytes(tmp);

                screen.draw_pixel(at.x + col_i, at.y + row_i, pixel);
            }
        }

        Ok(())
    }
}

pub struct Terminal {
    text: [[char; COLUMNS]; LINES],
    cursor: (usize, usize),
}

impl Terminal {
    pub fn new() -> Self {
        Terminal {
            text: [[char::default(); COLUMNS]; LINES],
            cursor: (0, 0),
        }
    }

    fn x(&self) -> usize {
        self.cursor.0
    }

    fn y(&self) -> usize {
        self.cursor.1
    }

    fn cursor_pos(&self) -> (usize, usize) {
        self.cursor
    }

    fn increment_cursor(&mut self) {
        self.cursor.0 += 1;

        if self.x() == COLUMNS {
            self.line_return();
        }
    }

    fn line_return(&mut self) {
        self.cursor.0 = 0;
        self.cursor.1 += 1;

        if self.y() == LINES {
            self.scroll_once();
            self.clear_line(self.y());
        }
    }

    fn carriage_return(&mut self) {
        self.clear_line(self.cursor.1);
        self.cursor.0 = 0;
    }

    fn clear_line(&mut self, line: usize) {
        self.text[line].fill(char::default());
    }

    fn move_line_up(&mut self, line: usize) {
        let mut tmp = [char::default(); COLUMNS];
        if line > 0 && line + 1 < LINES {
            tmp.copy_from_slice(&self.text[line + 1]);
            self.clear_line(line);
            self.text[line].copy_from_slice(&tmp)
        }
    }

    fn scroll_once(&mut self) {
        for line in 1..LINES {
            self.move_line_up(line);
        }
    }

    pub fn write_char(&mut self, c: char) {
        match c {
            '\r' => self.carriage_return(),
            '\n' => self.line_return(),
            _ => {
                self.text[self.y()][self.x()] = c;
                self.increment_cursor();
            }
        };
    }
}
