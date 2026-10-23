//! The bound on parse time (#15): no input may parse slower than 1 ms/KB
//! plus 1 ms. Upstream re-parses text to decide where things end, so some
//! inputs take polynomial or exponential time, such as unclosed blocks
//! nested in each other, and closing each such path one at a time can't
//! prove none is left. Fuel bounds them all: a parse gets fuel in
//! proportion to its input's length, and spends it on each exit matcher it
//! runs and each element or object it tries to parse. Once it is spent,
//! every exit matcher matches, so each parser in progress ends at once and
//! the parse fails with the opener that ran away.
//!
//! The weights make a unit cost about the same everywhere: 20–29 ns on an
//! M4 Max across the corpus, adversarial openers repeated to 100 KB, and
//! inputs that run away. Spending all the fuel therefore takes at most
//! about 0.7 µs per byte plus 0.6 ms, inside the bound even on an ubuntu-latest
//! GitHub runner, which parses at about the same speed (#48).
//!
//! The fuel lives in a thread-local because `OrgSource`, which every parser
//! advances through, is a plain `Copy` value with nowhere to keep it.

use std::cell::Cell;

/// Fuel per byte of input: twice what the corpus's densest large file
/// needs (12.2, ox-hugo's manual).
const PER_BYTE: u64 = 24;
/// Fuel every parse gets on top: three times what the corpus's neediest
/// small file spends in total (about 6,200).
const BASE: u64 = 20_000;

/// Spent per exit matcher run.
pub(crate) const EXIT_MATCHER: u64 = 1;
/// Spent per attempt to parse an element. Each tries some twenty parsers,
/// which mostly fail without running an exit matcher.
pub(crate) const ELEMENT: u64 = 24;
/// Spent per attempt to parse an object, for the same reason.
pub(crate) const OBJECT: u64 = 16;

#[derive(Clone, Copy)]
struct Tank {
    left: u64,
    /// Where the outermost element attempt in progress began.
    attempt_start: usize,
    attempt_depth: u32,
    /// Set when the fuel runs out: where the runaway parse began.
    opener: Option<usize>,
}

impl Tank {
    fn burn(&mut self, units: u64, offset: usize) {
        if self.opener.is_some() {
            return;
        }
        match self.left.checked_sub(units) {
            Some(left) => self.left = left,
            None => {
                self.left = 0;
                self.opener = Some(if self.attempt_depth > 0 {
                    self.attempt_start
                } else {
                    offset
                });
            }
        }
    }
}

thread_local! {
    // Unlimited until `fill`, so a parser called on its own in a unit test
    // never runs out.
    static TANK: Cell<Tank> = const {
        Cell::new(Tank {
            left: u64::MAX,
            attempt_start: 0,
            attempt_depth: 0,
            opener: None,
        })
    };
}

/// Gives this thread's next parse fuel for an input of `len` bytes.
pub(crate) fn fill(len: usize) {
    TANK.set(Tank {
        left: BASE.saturating_add((len as u64).saturating_mul(PER_BYTE)),
        attempt_start: 0,
        attempt_depth: 0,
        opener: None,
    });
}

/// Spends `units` of fuel on work at byte `offset`, and says whether the
/// fuel is spent.
pub(crate) fn burn(units: u64, offset: usize) -> bool {
    let mut tank = TANK.get();
    tank.burn(units, offset);
    TANK.set(tank);
    tank.opener.is_some()
}

pub(crate) fn is_spent() -> bool {
    TANK.get().opener.is_some()
}

/// Where the runaway parse began, if this thread's parse ran out of fuel.
pub(crate) fn opener() -> Option<usize> {
    TANK.get().opener
}

/// Marks an attempt to parse an element at `offset` until it is dropped.
/// The outermost one in progress is the opener a runaway parse reports:
/// the top-level paragraph whose objects ran away, or the first of the
/// unclosed blocks nested in each other.
pub(crate) struct Attempt;

impl Attempt {
    pub(crate) fn start(offset: usize) -> Attempt {
        let mut tank = TANK.get();
        if tank.attempt_depth == 0 {
            tank.attempt_start = offset;
        }
        tank.attempt_depth += 1;
        tank.burn(ELEMENT, offset);
        TANK.set(tank);
        Attempt
    }
}

impl Drop for Attempt {
    fn drop(&mut self) {
        let mut tank = TANK.get();
        tank.attempt_depth -= 1;
        TANK.set(tank);
    }
}
