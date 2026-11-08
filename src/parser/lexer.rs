use regex::Regex;
use std::sync::OnceLock;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum LType {
    Int,
    Decimal,
    Binary,
    Octodecimal,
    Hexadecimal,
    Scientific,
    Keyword,
    Id,
    Op,
}

pub type Pos = (
    usize, /* offset */
    usize, /* length */
);

pub struct Lexeme<'a>(pub LType, pub Pos, pub &'a str);

impl PartialEq<&str> for Lexeme<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.2 == *other
    }
}

impl std::default::Default for Lexeme<'_> {
    fn default() -> Self {
        Lexeme(LType::Id, (0,0), "")
    }
}

pub struct Lexer<'a> {
    str: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(str: &'a str) -> Self {
        Lexer {
            str,
            pos: 0,
        }
    }

    pub fn next_op(&mut self) -> Option<Lexeme<'a>> {
        self.advance(char::is_whitespace);

        let len = if let n = Self::is_multisym(self.leftover()) && n != 0 {
            n
        } else if self.leftover().starts_with(|c| char::is_ascii_punctuation(&c)) {
            1
        } else {
            return None
        };
        
        let (pos, str) = self.advance_by(len);
        Some(Lexeme(LType::Op, pos, str))
    }

    pub fn next(&mut self) -> Option<Lexeme<'a>> {
        self.advance(char::is_whitespace);

        let (mut typ, len) = if self.empty() {
            return None
        } else if let Some(num_or_id) = self.parse_number_or_id() {
            num_or_id
        } else if let n = Self::is_multisym(self.leftover()) && n != 0 {
            (LType::Op, n)
        } else {
            (LType::Op, 1)
        };
        
        let (pos, str) = self.advance_by(len);
        if let LType::Id = typ && Self::is_keyword(str) {
            typ = LType::Keyword
        }
        
        Some(Lexeme(typ, pos, str))
    }
    
    fn advance<P: Fn(char) -> bool>(&mut self, p: P) -> (Pos, &'a str) {
        self.advance_by(self.find(Self::not(p)))
    }

    fn advance_by(&mut self, n: usize) -> (Pos, &'a str) {
        self.pos += n;
        let advanced = &self.str[self.pos-n..self.pos];
        ((self.pos-n, n), advanced)
    }

    pub fn stepback(&mut self, str: &str) {
        // todo optimize: let some "stepback" attribute to avoid invoking regexp every time
        self.pos -= str.len()
    }

    pub fn empty(&mut self) -> bool {
        self.skipws();
        self.leftover().is_empty()
    }

    fn skipws(&mut self) {
        self.advance(char::is_whitespace);
    }

    fn parse_number_or_id(&self) -> Option<(LType, usize)> {
        static DECIMALS: OnceLock<[(LType, Regex); 7]> = OnceLock::new();
        let regexes = DECIMALS.get_or_init(|| {
            [
                (LType::Hexadecimal, Regex::new(r#"(?i)0x\S+"#).unwrap()),
                (LType::Octodecimal, Regex::new(r#"0o\S+"#).unwrap()),
                (LType::Binary, Regex::new(r#"0b\S+"#).unwrap()),
                (
                    LType::Scientific,
                    Regex::new(r#"(\d*\.\d+|\d+)[eE][+\-]?\d+"#).unwrap(),
                ),
                (LType::Decimal, Regex::new(r#"\d*\.\d+"#).unwrap()),
                (LType::Int, Regex::new(r#"\d+"#).unwrap()),
                (LType::Id, Regex::new(r#"[_a-zA-Z]\w*"#).unwrap()),
            ]
        });

        let str = self.leftover();
        for (notation, regex) in regexes {
            if let Some(m) = regex.find(str) && m.start() == 0 {
                return Some((*notation, m.len()));
            }
        }

        None
    }
    
    fn is_keyword(text: &str) -> bool {
        match text {
            "where" => true,
            _ => false,
        }
    }

    fn is_multisym(text: &str) -> usize {
        [">>=", "<<=", "..", ">=", "<=", "==", "!=", ">>", "<<"]
            .iter()
            .find(|m| text.starts_with(*m))
            .map(|m| m.len())
            .unwrap_or(0)
    }

    fn find<P: Fn(char) -> bool>(&self, p: P) -> usize {
        self.leftover()
            .find(p)
            .unwrap_or(self.leftover().len())
    }

    #[inline]
    fn leftover(&self) -> &'a str {
        &self.str[self.pos..]
    }

    /// Negate a predicate
    fn not<P: Fn(char) -> bool>(p: P) -> impl Fn(char) -> bool {
        move |c: char| !p(c)
    }
}

#[cfg(test)]
mod test_lexer {
    use super::*;

    fn collect(text: &str) -> Vec<&str> {
        let mut result = vec![];
        let mut lex = Lexer::new(text);
        while let Some(token) = lex.next() {
            result.push(token.2)
        }
        result
    }

    #[test]
    fn nothing() {
        assert_eq!(collect("").len(), 0)
    }

    #[test]
    fn classification() {
        let mut lex = Lexer::new("1 1.1 .1 0b1 0o1 0x1 1e0 1E5 1.1e1 abc where . >>");
        let mut want = |typ| assert_eq!(lex.next().unwrap().0, typ);
        want(LType::Int);
        want(LType::Decimal);
        want(LType::Decimal);
        want(LType::Binary);
        want(LType::Octodecimal);
        want(LType::Hexadecimal);
        want(LType::Scientific);
        want(LType::Scientific);
        want(LType::Scientific);
        want(LType::Id);
        want(LType::Keyword);
        want(LType::Op);
        want(LType::Op);
        assert!(lex.next().is_none())
    }

    #[test]
    fn different_tokens() {
        assert_eq!(collect("1 a _ +( )"), vec!["1", "a", "_", "+", "(", ")"])
    }

    #[test]
    fn multicharacter_tokens() {
        assert_eq!(
            collect("a123..123 >= _abc"),
            vec!["a123", "..", "123", ">=", "_abc"]
        )
    }

    #[test]
    fn whitecharacter_break() {
        assert_eq!(collect("12 34\n3x"), vec!["12", "34", "3", "x"])
    }

    #[test]
    fn numbers() {
        assert_eq!(
            collect("1 23 0xdeadcode 0o777 0b01110 1e6 1e+6 1e+6+5 1e-6 1E6 1.5 12.34"),
            vec![
                "1",
                "23",
                "0xdeadcode",
                "0o777",
                "0b01110",
                "1e6",
                "1e+6",
                "1e+6",
                "+",
                "5",
                "1e-6",
                "1E6",
                "1.5",
                "12.34"
            ]
        );
        assert_eq!(collect("1. .1 .1"), vec!["1", ".", ".1", ".1"]);
        assert_eq!(
            collect("1e 1e- 1e+"),
            vec!["1", "e", "1", "e", "-", "1", "e", "+"]
        );
        assert_eq!(
            collect("1..3 1.1..2.2"),
            vec!["1", "..", "3", "1.1", "..", "2.2"]
        )
    }

    #[test]
    fn lambda_body_delimiter_case() {
        // test whether "." in \x.5 can be disambiguated as a body splitter, not as a decimal .5
        let mut lex = Lexer::new("x.5");
        assert_eq!(lex.next().unwrap().0, LType::Id);
        assert_eq!(lex.next_op().unwrap().2, ".");
        assert_eq!(lex.next().unwrap().0, LType::Int);
    }
}
