use eyre::{eyre, Result};

#[derive(Debug, PartialEq, Eq)]
pub enum Lexeme<'t> {
    Str(&'t str),
    Qoutes,
    OpenBracket,
    CloseBracket,
    /// Let the caller decide if they need an int or a float
    Num(&'t str),
    Eof,
}

impl<'t> Lexeme<'t> {
    pub fn unwrap_str(&self) -> &'t str {
        match self {
            Lexeme::Str(stri) => stri,
            _ => panic!(),
        }
    }

    pub fn unwrap_num(&self) -> &'t str {
        match self {
            Lexeme::Num(num) => num,
            _ => panic!(),
        }
    }
}

pub struct Lexer<'t> {
    txt: &'t str,
    lexeme_buf: Option<Lexeme<'t>>,
}

impl<'t> Lexer<'t> {
    pub fn new(txt: &'t str) -> Self {
        Self {
            txt,
            lexeme_buf: None,
        }
    }

    pub fn peek(&mut self) -> Result<&Lexeme<'t>> {
        if self.lexeme_buf.is_some() {
            Ok(self.lexeme_buf.as_ref().unwrap())
        } else {
            let l = self.next()?;
            self.lexeme_buf = Some(l);
            Ok(self.lexeme_buf.as_ref().unwrap())
        }
    }

    pub fn next(&mut self) -> Result<Lexeme<'t>> {
        if let Some(l) = self.lexeme_buf.take() {
            return Ok(l);
        }

        let next = self.peek_char();

        if let Some(next) = next {
            if next == '#' || next.is_ascii_whitespace() {
                self.skip_whitespace_comments();
            }
        };

        let next = self.peek_char();

        Ok(match next {
            Some(ch) => match ch {
                '"' => {
                    self.advance();
                    Lexeme::Qoutes
                }
                '[' => {
                    self.advance();
                    Lexeme::OpenBracket
                }
                ']' => {
                    self.advance();
                    Lexeme::CloseBracket
                }
                ch if ch.is_alphabetic() => self.lex_str(),
                '-' | '.' => self.lex_num(),
                ch if ch.is_ascii_digit() => self.lex_num(),
                ch => return Err(eyre!("Invalid character: '{}'", ch)),
            },
            None => Lexeme::Eof,
        })
    }

    fn lex_str(&mut self) -> Lexeme<'t> {
        let s = self.advance_while(|ch| ch != '\"' && !ch.is_whitespace());
        Lexeme::Str(s)
    }

    fn lex_num(&mut self) -> Lexeme<'t> {
        let s = self.advance_while(|ch| ch == '-' || ch == '.' || ch == 'e' || ch.is_ascii_digit());
        Lexeme::Num(s)
    }

    fn skip_whitespace_comments(&mut self) {
        while let Some(ch) = self.peek_char() {
            match ch {
                '#' => {
                    self.advance_while(|ch| ch != '\n');
                    self.advance();
                    continue;
                }
                ch if ch.is_ascii_whitespace() => {
                    self.advance_while(|ch| ch.is_ascii_whitespace());
                }
                _ => break,
            }
        }
    }

    fn advance_while(&mut self, cond: fn(char) -> bool) -> &'t str {
        let mut index = 0;
        while self
            .txt
            .as_bytes()
            .get(index)
            .map(|ch| cond(*ch as char))
            .unwrap_or(false)
        {
            index += 1;
        }

        if index == 0 {
            panic!("Lexer implementation error");
        }

        let (s, rest) = self.txt.split_at(index);
        self.txt = rest;

        s
    }

    fn advance(&mut self) -> Option<char> {
        let c = match self.peek_char() {
            Some(c) => c,
            None => panic!("Lexer implementation error"),
        };

        self.txt = &self.txt[1..];

        Some(c)
    }

    fn peek_char(&mut self) -> Option<char> {
        self.txt.as_bytes().first().map(|ch| *ch as char)
    }
}

#[cfg(test)]
mod test_super {
    use super::{Lexeme, Lexer};

    #[test]
    fn test_example_1() {
        let input = "LookAt 3 4 1.5  # eye
        .5 .5 0  # look at point
        0 0 1    # up vector
        Camera \"perspective\" \"float fov\" 45";

        let mut lexer = Lexer::new(&input);

        assert_eq!(lexer.next().unwrap(), Lexeme::Str("LookAt"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Num("3"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Num("4"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Num("1.5"));

        assert_eq!(lexer.next().unwrap(), Lexeme::Num(".5"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Num(".5"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Num("0"));

        assert_eq!(lexer.next().unwrap(), Lexeme::Num("0"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Num("0"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Num("1"));

        assert_eq!(lexer.next().unwrap(), Lexeme::Str("Camera"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Qoutes);
        assert_eq!(lexer.next().unwrap(), Lexeme::Str("perspective"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Qoutes);
        assert_eq!(lexer.next().unwrap(), Lexeme::Qoutes);
        assert_eq!(lexer.next().unwrap(), Lexeme::Str("float"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Str("fov"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Qoutes);
        assert_eq!(lexer.next().unwrap(), Lexeme::Num("45"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Eof);
    }

    #[test]
    fn test_example_2() {
        let input = "Texture \"checks\" \"spectrum\" \"checkerboard\"
        \"float uscale\" [16] \"float vscale\" [16]
        \"rgb tex1\" [.1 .1 .1] \"rgb tex2\" [.8 .8 .8]";

        let mut lexer = Lexer::new(&input);

        assert_eq!(lexer.next().unwrap(), Lexeme::Str("Texture"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Qoutes);
        assert_eq!(lexer.next().unwrap(), Lexeme::Str("checks"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Qoutes);
        assert_eq!(lexer.next().unwrap(), Lexeme::Qoutes);
        assert_eq!(lexer.next().unwrap(), Lexeme::Str("spectrum"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Qoutes);
        assert_eq!(lexer.next().unwrap(), Lexeme::Qoutes);
        assert_eq!(lexer.next().unwrap(), Lexeme::Str("checkerboard"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Qoutes);

        assert_eq!(lexer.next().unwrap(), Lexeme::Qoutes);
        assert_eq!(lexer.next().unwrap(), Lexeme::Str("float"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Str("uscale"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Qoutes);
        assert_eq!(lexer.next().unwrap(), Lexeme::OpenBracket);
        assert_eq!(lexer.next().unwrap(), Lexeme::Num("16"));
        assert_eq!(lexer.next().unwrap(), Lexeme::CloseBracket);
        assert_eq!(lexer.next().unwrap(), Lexeme::Qoutes);
        assert_eq!(lexer.next().unwrap(), Lexeme::Str("float"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Str("vscale"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Qoutes);
        assert_eq!(lexer.next().unwrap(), Lexeme::OpenBracket);
        assert_eq!(lexer.next().unwrap(), Lexeme::Num("16"));
        assert_eq!(lexer.next().unwrap(), Lexeme::CloseBracket);

        assert_eq!(lexer.next().unwrap(), Lexeme::Qoutes);
        assert_eq!(lexer.next().unwrap(), Lexeme::Str("rgb"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Str("tex1"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Qoutes);
        assert_eq!(lexer.next().unwrap(), Lexeme::OpenBracket);
        assert_eq!(lexer.next().unwrap(), Lexeme::Num(".1"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Num(".1"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Num(".1"));
        assert_eq!(lexer.next().unwrap(), Lexeme::CloseBracket);
        assert_eq!(lexer.next().unwrap(), Lexeme::Qoutes);
        assert_eq!(lexer.next().unwrap(), Lexeme::Str("rgb"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Str("tex2"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Qoutes);
        assert_eq!(lexer.next().unwrap(), Lexeme::OpenBracket);
        assert_eq!(lexer.next().unwrap(), Lexeme::Num(".8"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Num(".8"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Num(".8"));
        assert_eq!(lexer.next().unwrap(), Lexeme::CloseBracket);
        assert_eq!(lexer.next().unwrap(), Lexeme::Eof);
    }

    #[test]
    fn test_comments() {
        let input = "#
        Camera
        # dsds dsdsdsd s ds sdd s Sampler
        #     
        WorldBegin";

        let mut lexer = Lexer::new(input);

        assert_eq!(lexer.next().unwrap(), Lexeme::Str("Camera"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Str("WorldBegin"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Eof);
    }

    #[test]
    fn test_floats_exp() {
        let input = "4.37114e-8 1 1.91069e-15";
        let mut lexer = Lexer::new(input);
        assert_eq!(lexer.next().unwrap(), Lexeme::Num("4.37114e-8"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Num("1"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Num("1.91069e-15"));
        assert_eq!(lexer.next().unwrap(), Lexeme::Eof);
    }
}
