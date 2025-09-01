use std::io::{self, Write};

pub use crossterm;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::NoTtyEvent;
#[cfg(feature = "underline-color")]
use crossterm::style::SetUnderlineColor;
use crossterm::style::{
    Attribute as CrosstermAttribute, Attributes as CrosstermAttributes, Color as CrosstermColor,
    Colors as CrosstermColors, ContentStyle, Print, SetAttribute, SetBackgroundColor, SetColors,
    SetForegroundColor,
};
use crossterm::terminal::{self, Clear};
use crossterm::{execute, queue};
use ratatui_core::backend::{Backend, ClearType, WindowSize};
use ratatui_core::buffer::Cell;
use ratatui_core::layout::{Position, Size};
use ratatui_core::style::{Color, Modifier, Style};

#[derive(Clone)]
pub struct NottyBackend<W: Write> {
    term: NoTtyEvent,
    /// The writer used to send commands to the terminal.
    writer: W,
}

impl<W> NottyBackend<W>
where
    W: Write,
{
    pub const fn new(term: NoTtyEvent, writer: W) -> Self {
        Self { term, writer }
    }

    pub const fn writer(&self) -> &W {
        &self.writer
    }

    pub const fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }
}

impl<W> Write for NottyBackend<W>
where
    W: Write,
{
    /// Writes a buffer of bytes to the underlying buffer.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writer.write(buf)
    }

    /// Flushes the underlying buffer.
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

impl<W> Backend for NottyBackend<W>
where
    W: Write,
{
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let mut fg = Color::Reset;
        let mut bg = Color::Reset;
        #[cfg(feature = "underline-color")]
        let mut underline_color = Color::Reset;
        let mut modifier = Modifier::empty();
        let mut last_pos: Option<Position> = None;
        for (x, y, cell) in content {
            // Move the cursor if the previous location was not (x - 1, y)
            if !matches!(last_pos, Some(p) if x == p.x + 1 && y == p.y) {
                queue!(self.writer, MoveTo(x, y))?;
            }
            last_pos = Some(Position { x, y });
            if cell.modifier != modifier {
                let diff = ModifierDiff {
                    from: modifier,
                    to: cell.modifier,
                };
                diff.queue(&mut self.writer)?;
                modifier = cell.modifier;
            }
            if cell.fg != fg || cell.bg != bg {
                queue!(
                    self.writer,
                    SetColors(CrosstermColors::new(
                        cell.fg.into_notty(),
                        cell.bg.into_notty(),
                    ))
                )?;
                fg = cell.fg;
                bg = cell.bg;
            }
            #[cfg(feature = "underline-color")]
            if cell.underline_color != underline_color {
                let color = cell.underline_color.into_notty();
                queue!(self.writer, SetUnderlineColor(color))?;
                underline_color = cell.underline_color;
            }

            queue!(self.writer, Print(cell.symbol()))?;
        }

        #[cfg(feature = "underline-color")]
        return queue!(
            self.writer,
            SetForegroundColor(CrosstermColor::Reset),
            SetBackgroundColor(CrosstermColor::Reset),
            SetUnderlineColor(CrosstermColor::Reset),
            SetAttribute(CrosstermAttribute::Reset),
        );
        #[cfg(not(feature = "underline-color"))]
        return queue!(
            self.writer,
            SetForegroundColor(CrosstermColor::Reset),
            SetBackgroundColor(CrosstermColor::Reset),
            SetAttribute(CrosstermAttribute::Reset),
        );
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        execute!(self.writer, Hide)
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        execute!(self.writer, Show)
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        crossterm::cursor::position(&self.term)
            .map(|(x, y)| Position { x, y })
            .map_err(io::Error::other)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        let Position { x, y } = position.into();
        execute!(self.writer, MoveTo(x, y))
    }

    fn clear(&mut self) -> io::Result<()> {
        self.clear_region(ClearType::All)
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        execute!(
            self.writer,
            Clear(match clear_type {
                ClearType::All => crossterm::terminal::ClearType::All,
                ClearType::AfterCursor => crossterm::terminal::ClearType::FromCursorDown,
                ClearType::BeforeCursor => crossterm::terminal::ClearType::FromCursorUp,
                ClearType::CurrentLine => crossterm::terminal::ClearType::CurrentLine,
                ClearType::UntilNewLine => crossterm::terminal::ClearType::UntilNewLine,
            })
        )
    }

    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        for _ in 0..n {
            queue!(self.writer, Print("\n"))?;
        }
        self.writer.flush()
    }

    fn size(&self) -> io::Result<Size> {
        let (width, height) = terminal::size(&self.term)?;
        Ok(Size { width, height })
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        let crossterm::terminal::WindowSize {
            columns,
            rows,
            width,
            height,
        } = terminal::window_size(&self.term)?;
        Ok(WindowSize {
            columns_rows: Size {
                width: columns,
                height: rows,
            },
            pixels: Size { width, height },
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    #[cfg(feature = "scrolling-regions")]
    fn scroll_region_up(&mut self, region: std::ops::Range<u16>, amount: u16) -> io::Result<()> {
        queue!(
            self.writer,
            ScrollUpInRegion {
                first_row: region.start,
                last_row: region.end.saturating_sub(1),
                lines_to_scroll: amount,
            }
        )?;
        self.writer.flush()
    }

    #[cfg(feature = "scrolling-regions")]
    fn scroll_region_down(&mut self, region: std::ops::Range<u16>, amount: u16) -> io::Result<()> {
        queue!(
            self.writer,
            ScrollDownInRegion {
                first_row: region.start,
                last_row: region.end.saturating_sub(1),
                lines_to_scroll: amount,
            }
        )?;
        self.writer.flush()
    }
}

/// A trait for converting a Ratatui type to a Crossterm type.
///
/// This trait is needed for avoiding the orphan rule when implementing `From` for crossterm types
/// once these are moved to a separate crate.
pub trait IntoNotty<C> {
    /// Converts the ratatui type to a crossterm type.
    fn into_notty(self) -> C;
}

/// A trait for converting a Crossterm type to a Ratatui type.
///
/// This trait is needed for avoiding the orphan rule when implementing `From` for crossterm types
/// once these are moved to a separate crate.
pub trait FromNotty<C> {
    /// Converts the crossterm type to a ratatui type.
    fn from_notty(value: C) -> Self;
}

impl IntoNotty<CrosstermColor> for Color {
    fn into_notty(self) -> CrosstermColor {
        match self {
            Self::Reset => CrosstermColor::Reset,
            Self::Black => CrosstermColor::Black,
            Self::Red => CrosstermColor::DarkRed,
            Self::Green => CrosstermColor::DarkGreen,
            Self::Yellow => CrosstermColor::DarkYellow,
            Self::Blue => CrosstermColor::DarkBlue,
            Self::Magenta => CrosstermColor::DarkMagenta,
            Self::Cyan => CrosstermColor::DarkCyan,
            Self::Gray => CrosstermColor::Grey,
            Self::DarkGray => CrosstermColor::DarkGrey,
            Self::LightRed => CrosstermColor::Red,
            Self::LightGreen => CrosstermColor::Green,
            Self::LightBlue => CrosstermColor::Blue,
            Self::LightYellow => CrosstermColor::Yellow,
            Self::LightMagenta => CrosstermColor::Magenta,
            Self::LightCyan => CrosstermColor::Cyan,
            Self::White => CrosstermColor::White,
            Self::Indexed(i) => CrosstermColor::AnsiValue(i),
            Self::Rgb(r, g, b) => CrosstermColor::Rgb { r, g, b },
        }
    }
}

impl FromNotty<CrosstermColor> for Color {
    fn from_notty(value: CrosstermColor) -> Self {
        match value {
            CrosstermColor::Reset => Self::Reset,
            CrosstermColor::Black => Self::Black,
            CrosstermColor::DarkRed => Self::Red,
            CrosstermColor::DarkGreen => Self::Green,
            CrosstermColor::DarkYellow => Self::Yellow,
            CrosstermColor::DarkBlue => Self::Blue,
            CrosstermColor::DarkMagenta => Self::Magenta,
            CrosstermColor::DarkCyan => Self::Cyan,
            CrosstermColor::Grey => Self::Gray,
            CrosstermColor::DarkGrey => Self::DarkGray,
            CrosstermColor::Red => Self::LightRed,
            CrosstermColor::Green => Self::LightGreen,
            CrosstermColor::Blue => Self::LightBlue,
            CrosstermColor::Yellow => Self::LightYellow,
            CrosstermColor::Magenta => Self::LightMagenta,
            CrosstermColor::Cyan => Self::LightCyan,
            CrosstermColor::White => Self::White,
            CrosstermColor::Rgb { r, g, b } => Self::Rgb(r, g, b),
            CrosstermColor::AnsiValue(v) => Self::Indexed(v),
        }
    }
}

/// The `ModifierDiff` struct is used to calculate the difference between two `Modifier`
/// values. This is useful when updating the terminal display, as it allows for more
/// efficient updates by only sending the necessary changes.
struct ModifierDiff {
    pub from: Modifier,
    pub to: Modifier,
}

impl ModifierDiff {
    fn queue<W>(self, mut w: W) -> io::Result<()>
    where
        W: io::Write,
    {
        //use crossterm::Attribute;
        let removed = self.from - self.to;
        if removed.contains(Modifier::REVERSED) {
            queue!(w, SetAttribute(CrosstermAttribute::NoReverse))?;
        }
        if removed.contains(Modifier::BOLD) || removed.contains(Modifier::DIM) {
            // Bold and Dim are both reset by applying the Normal intensity
            queue!(w, SetAttribute(CrosstermAttribute::NormalIntensity))?;

            // The remaining Bold and Dim attributes must be
            // reapplied after the intensity reset above.
            if self.to.contains(Modifier::DIM) {
                queue!(w, SetAttribute(CrosstermAttribute::Dim))?;
            }

            if self.to.contains(Modifier::BOLD) {
                queue!(w, SetAttribute(CrosstermAttribute::Bold))?;
            }
        }
        if removed.contains(Modifier::ITALIC) {
            queue!(w, SetAttribute(CrosstermAttribute::NoItalic))?;
        }
        if removed.contains(Modifier::UNDERLINED) {
            queue!(w, SetAttribute(CrosstermAttribute::NoUnderline))?;
        }
        if removed.contains(Modifier::CROSSED_OUT) {
            queue!(w, SetAttribute(CrosstermAttribute::NotCrossedOut))?;
        }
        if removed.contains(Modifier::SLOW_BLINK) || removed.contains(Modifier::RAPID_BLINK) {
            queue!(w, SetAttribute(CrosstermAttribute::NoBlink))?;
        }

        let added = self.to - self.from;
        if added.contains(Modifier::REVERSED) {
            queue!(w, SetAttribute(CrosstermAttribute::Reverse))?;
        }
        if added.contains(Modifier::BOLD) {
            queue!(w, SetAttribute(CrosstermAttribute::Bold))?;
        }
        if added.contains(Modifier::ITALIC) {
            queue!(w, SetAttribute(CrosstermAttribute::Italic))?;
        }
        if added.contains(Modifier::UNDERLINED) {
            queue!(w, SetAttribute(CrosstermAttribute::Underlined))?;
        }
        if added.contains(Modifier::DIM) {
            queue!(w, SetAttribute(CrosstermAttribute::Dim))?;
        }
        if added.contains(Modifier::CROSSED_OUT) {
            queue!(w, SetAttribute(CrosstermAttribute::CrossedOut))?;
        }
        if added.contains(Modifier::SLOW_BLINK) {
            queue!(w, SetAttribute(CrosstermAttribute::SlowBlink))?;
        }
        if added.contains(Modifier::RAPID_BLINK) {
            queue!(w, SetAttribute(CrosstermAttribute::RapidBlink))?;
        }

        Ok(())
    }
}

impl FromNotty<CrosstermAttribute> for Modifier {
    fn from_notty(value: CrosstermAttribute) -> Self {
        // `Attribute*s*` (note the *s*) contains multiple `Attribute` We convert `Attribute` to
        // `Attribute*s*` (containing only 1 value) to avoid implementing the conversion again
        Self::from_notty(CrosstermAttributes::from(value))
    }
}

impl FromNotty<CrosstermAttributes> for Modifier {
    fn from_notty(value: CrosstermAttributes) -> Self {
        let mut res = Self::empty();
        if value.has(CrosstermAttribute::Bold) {
            res |= Self::BOLD;
        }
        if value.has(CrosstermAttribute::Dim) {
            res |= Self::DIM;
        }
        if value.has(CrosstermAttribute::Italic) {
            res |= Self::ITALIC;
        }
        if value.has(CrosstermAttribute::Underlined)
            || value.has(CrosstermAttribute::DoubleUnderlined)
            || value.has(CrosstermAttribute::Undercurled)
            || value.has(CrosstermAttribute::Underdotted)
            || value.has(CrosstermAttribute::Underdashed)
        {
            res |= Self::UNDERLINED;
        }
        if value.has(CrosstermAttribute::SlowBlink) {
            res |= Self::SLOW_BLINK;
        }
        if value.has(CrosstermAttribute::RapidBlink) {
            res |= Self::RAPID_BLINK;
        }
        if value.has(CrosstermAttribute::Reverse) {
            res |= Self::REVERSED;
        }
        if value.has(CrosstermAttribute::Hidden) {
            res |= Self::HIDDEN;
        }
        if value.has(CrosstermAttribute::CrossedOut) {
            res |= Self::CROSSED_OUT;
        }
        res
    }
}

impl FromNotty<ContentStyle> for Style {
    fn from_notty(value: ContentStyle) -> Self {
        let mut sub_modifier = Modifier::empty();
        if value.attributes.has(CrosstermAttribute::NoBold) {
            sub_modifier |= Modifier::BOLD;
        }
        if value.attributes.has(CrosstermAttribute::NoItalic) {
            sub_modifier |= Modifier::ITALIC;
        }
        if value.attributes.has(CrosstermAttribute::NotCrossedOut) {
            sub_modifier |= Modifier::CROSSED_OUT;
        }
        if value.attributes.has(CrosstermAttribute::NoUnderline) {
            sub_modifier |= Modifier::UNDERLINED;
        }
        if value.attributes.has(CrosstermAttribute::NoHidden) {
            sub_modifier |= Modifier::HIDDEN;
        }
        if value.attributes.has(CrosstermAttribute::NoBlink) {
            sub_modifier |= Modifier::RAPID_BLINK | Modifier::SLOW_BLINK;
        }
        if value.attributes.has(CrosstermAttribute::NoReverse) {
            sub_modifier |= Modifier::REVERSED;
        }

        Self {
            fg: value.foreground_color.map(FromNotty::from_notty),
            bg: value.background_color.map(FromNotty::from_notty),
            #[cfg(feature = "underline-color")]
            underline_color: value.underline_color.map(FromNotty::from_notty),
            add_modifier: Modifier::from_notty(value.attributes),
            sub_modifier,
        }
    }
}

/// A command that scrolls the terminal screen a given number of rows up in a specific scrolling
/// region.
///
/// This will hopefully be replaced by a struct in crossterm proper. There are two outstanding
/// crossterm PRs that will address this:
///   - [918](https://github.com/crossterm-rs/crossterm/pull/918)
///   - [923](https://github.com/crossterm-rs/crossterm/pull/923)
#[cfg(feature = "scrolling-regions")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScrollUpInRegion {
    /// The first row of the scrolling region.
    pub first_row: u16,

    /// The last row of the scrolling region.
    pub last_row: u16,

    /// The number of lines to scroll up by.
    pub lines_to_scroll: u16,
}

#[cfg(feature = "scrolling-regions")]
impl crate::crossterm::Command for ScrollUpInRegion {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        if self.lines_to_scroll != 0 {
            // Set a scrolling region that contains just the desired lines.
            write!(
                f,
                crate::crossterm::csi!("{};{}r"),
                self.first_row.saturating_add(1),
                self.last_row.saturating_add(1)
            )?;
            // Scroll the region by the desired count.
            write!(f, crate::crossterm::csi!("{}S"), self.lines_to_scroll)?;
            // Reset the scrolling region to be the whole screen.
            write!(f, crate::crossterm::csi!("r"))?;
        }
        Ok(())
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "ScrollUpInRegion command not supported for winapi",
        ))
    }
}

/// A command that scrolls the terminal screen a given number of rows down in a specific scrolling
/// region.
///
/// This will hopefully be replaced by a struct in crossterm proper. There are two outstanding
/// crossterm PRs that will address this:
///   - [918](https://github.com/crossterm-rs/crossterm/pull/918)
///   - [923](https://github.com/crossterm-rs/crossterm/pull/923)
#[cfg(feature = "scrolling-regions")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScrollDownInRegion {
    /// The first row of the scrolling region.
    pub first_row: u16,

    /// The last row of the scrolling region.
    pub last_row: u16,

    /// The number of lines to scroll down by.
    pub lines_to_scroll: u16,
}

#[cfg(feature = "scrolling-regions")]
impl crate::crossterm::Command for ScrollDownInRegion {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        if self.lines_to_scroll != 0 {
            // Set a scrolling region that contains just the desired lines.
            write!(
                f,
                crate::crossterm::csi!("{};{}r"),
                self.first_row.saturating_add(1),
                self.last_row.saturating_add(1)
            )?;
            // Scroll the region by the desired count.
            write!(f, crate::crossterm::csi!("{}T"), self.lines_to_scroll)?;
            // Reset the scrolling region to be the whole screen.
            write!(f, crate::crossterm::csi!("r"))?;
        }
        Ok(())
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "ScrollDownInRegion command not supported for winapi",
        ))
    }
}
