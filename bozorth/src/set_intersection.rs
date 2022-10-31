use std::iter::Peekable;

pub(crate) struct Intersection<T, I, J>
where
    T: Eq + Ord,
    I: Iterator<Item = T>,
    J: Iterator<Item = T>,
{
    first: Peekable<I>,
    second: Peekable<J>,
}

impl<T, I, J> Iterator for Intersection<T, I, J>
where
    T: Eq + Ord,
    I: Iterator<Item = T>,
    J: Iterator<Item = T>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        while let (Some(a), Some(b)) = (self.first.peek(), self.second.peek()) {
            use std::cmp::Ordering::*;

            match a.cmp(b) {
                Greater => {
                    self.second.next();
                }
                Equal => {
                    self.first.next();
                    return self.second.next();
                }
                Less => {
                    self.first.next();
                }
            }
        }
        None
    }
}

pub(crate) fn intersection_of_sorted<T, I, J>(first: I, second: J) -> Intersection<T, I, J>
where
    T: Eq + Ord,
    I: Iterator<Item = T>,
    J: Iterator<Item = T>,
{
    Intersection {
        first: first.peekable(),
        second: second.peekable(),
    }
}

#[cfg(test)]
mod tests {
    use crate::set_intersection::intersection_of_sorted;

    #[test]
    fn simple() {
        let a = 2..10;
        let b = 3..5;

        let mut c = intersection_of_sorted(a, b);
        assert_eq!(c.next(), Some(3));
        assert_eq!(c.next(), Some(4));
        assert_eq!(c.next(), None);
    }
}
