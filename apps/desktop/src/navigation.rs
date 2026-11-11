//! View-local navigation; it does not modify document contents.
#[derive(Clone, Copy)]
pub enum Direction {
    First,
    Previous,
    Next,
    Last,
}

#[derive(Default)]
pub struct Navigation {
    /// Zero-based page at the viewport's current reading position.
    pub current: u16,
    pub count: u16,
}

impl Navigation {
    pub fn target(&self, direction: Direction) -> Option<u16> {
        if self.count == 0 {
            return None;
        }
        let target = match direction {
            Direction::First => 0,
            Direction::Previous => self.current.checked_sub(1)?,
            Direction::Next => self.current.checked_add(1)?,
            Direction::Last => self.count - 1,
        };
        (target < self.count && target != self.current).then_some(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_single_page_documents_have_no_navigation() {
        for count in [0, 1] {
            let nav = Navigation { current: 0, count };
            for direction in [
                Direction::First,
                Direction::Previous,
                Direction::Next,
                Direction::Last,
            ] {
                assert_eq!(nav.target(direction), None);
            }
        }
    }

    #[test]
    fn every_page_can_be_reached_in_both_directions_without_crossing_bounds() {
        let mut nav = Navigation {
            current: 0,
            count: 100,
        };
        assert_eq!(nav.target(Direction::Previous), None);
        for expected in 1..100 {
            assert_eq!(nav.target(Direction::Next), Some(expected));
            nav.current = expected;
        }
        assert_eq!(nav.target(Direction::Next), None);
        assert_eq!(nav.target(Direction::First), Some(0));
        for expected in (0..99).rev() {
            assert_eq!(nav.target(Direction::Previous), Some(expected));
            nav.current = expected;
        }
        assert_eq!(nav.target(Direction::Last), Some(99));
    }
}
