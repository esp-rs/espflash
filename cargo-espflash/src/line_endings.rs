// Adapted from https://github.com/derekdreery/normalize-line-endings

struct Normalized<I> {
    iter: I,
    prev_was_cr: bool,
    peeked: Option<u8>,
}

pub fn normalized(iter: impl Iterator<Item = u8>) -> impl Iterator<Item = u8> {
    Normalized {
        iter,
        prev_was_cr: false,
        peeked: None,
    }
}

impl<I> Iterator for Normalized<I>
where
    I: Iterator<Item = u8>,
{
    type Item = u8;
    fn next(&mut self) -> Option<u8> {
        if let Some(peeked) = self.peeked.take() {
            return Some(peeked);
        }
        match self.iter.next() {
            Some(b'\n') if !self.prev_was_cr => {
                self.peeked = Some(b'\n');
                self.prev_was_cr = false;
                Some(b'\r')
            }
            Some(b'\r') => {
                self.prev_was_cr = true;
                Some(b'\r')
            }
            any => {
                self.prev_was_cr = false;
                any
            }
        }
    }
}

// tests
#[cfg(test)]
mod tests {
    use std::iter::FromIterator;

    #[test]
    fn normalized() {
        let input = b"This is a string \n with \n some \n\r\n random newlines\r\n\n";
        assert_eq!(
            &Vec::from_iter(super::normalized(input.iter().copied())),
            b"This is a string \r\n with \r\n some \r\n\r\n random newlines\r\n\r\n"
        );
    }
}
