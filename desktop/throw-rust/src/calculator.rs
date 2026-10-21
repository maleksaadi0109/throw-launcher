//! Bounded arithmetic parser. Never evaluates scripts or invokes a shell.
pub fn evaluate(input: &str) -> Option<f64> {
    let input = input
        .trim()
        .strip_prefix('=')
        .unwrap_or(input.trim())
        .trim();
    if input.is_empty() || input.len() > 256 {
        return None;
    }
    let mut parser = Parser {
        bytes: input.as_bytes(),
        at: 0,
        depth: 0,
    };
    let value = parser.expression()?;
    parser.space();
    (parser.at == parser.bytes.len() && value.is_finite()).then_some(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
    depth: usize,
}

impl Parser<'_> {
    fn space(&mut self) {
        while self.bytes.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
    }
    fn take(&mut self, byte: u8) -> bool {
        self.space();
        if self.bytes.get(self.at) == Some(&byte) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn expression(&mut self) -> Option<f64> {
        let mut value = self.term()?;
        loop {
            if self.take(b'+') {
                value += self.term()?;
            } else if self.take(b'-') {
                value -= self.term()?;
            } else {
                return value.is_finite().then_some(value);
            }
        }
    }
    fn term(&mut self) -> Option<f64> {
        let mut value = self.atom()?;
        loop {
            if self.take(b'*') {
                value *= self.atom()?;
            } else if self.take(b'/') {
                let right = self.atom()?;
                if right == 0.0 {
                    return None;
                }
                value /= right;
            } else {
                return value.is_finite().then_some(value);
            }
        }
    }
    fn atom(&mut self) -> Option<f64> {
        self.depth += 1;
        if self.depth > 24 {
            return None;
        }
        let result = if self.take(b'+') {
            self.atom()
        } else if self.take(b'-') {
            self.atom().map(|v| -v)
        } else if self.take(b'(') {
            let value = self.expression()?;
            if !self.take(b')') {
                return None;
            }
            Some(value)
        } else {
            self.space();
            let start = self.at;
            while self
                .bytes
                .get(self.at)
                .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
            {
                self.at += 1;
            }
            std::str::from_utf8(&self.bytes[start..self.at])
                .ok()?
                .parse::<f64>()
                .ok()
        };
        self.depth -= 1;
        result.filter(|v| v.is_finite())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn precedence_and_parentheses() {
        assert_eq!(evaluate("= (12 + 3) * 2 / 5"), Some(6.0));
        assert_eq!(evaluate("2+3*4"), Some(14.0));
        assert_eq!(evaluate("-2 * -4.5"), Some(9.0));
    }
    #[test]
    fn rejects_code_invalid_and_unbounded_expressions() {
        for input in [
            "1/0",
            "NaN",
            "inf",
            "2 +",
            "2; calc.exe",
            "2^3",
            "1 2",
            "",
            "()",
        ] {
            assert_eq!(evaluate(input), None, "{input}");
        }
        assert_eq!(
            evaluate(&format!("{}1{}", "(".repeat(30), ")".repeat(30))),
            None
        );
        assert_eq!(evaluate(&"1".repeat(257)), None);
    }
}
