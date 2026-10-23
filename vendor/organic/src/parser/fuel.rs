//! The bound on parse time (#15): no input may parse slower than 1 ms/KB
//! plus 1 ms. Upstream re-parses text to decide where things end, so some
//! inputs take polynomial or exponential time, such as unclosed blocks
//! nested in each other, and closing each such path one at a time can't
//! prove none is left. Fuel bounds them all: a parse gets fuel in
//! proportion to its input's length, and spends it on each exit matcher it
//! runs and each element, list item, table cell or object it tries to
//! parse. Once it is spent, every exit matcher matches and every attempt
//! fails with a nom `Failure`, which `alt`, `opt` and `many0` pass straight
//! up instead of trying an alternative; exits alone left nested openers
//! backtracking through exponentially many alternatives. The parse then
//! fails with the opener that ran away.
//!
//! Calibrated on an M4 Max, in release (#48):
//! - The corpus needs at most 20.5 units a byte in a large file (ox-hugo's
//!   manual) and 9,656 in a small one (worg's babel finances example); the
//!   limits below are 1.8 and 2.1 times that.
//! - Inputs that run out spend 7–23 ns a unit, among them every slow input
//!   the fuzz targets found, so running out takes at most about 0.8 µs a
//!   byte plus 0.5 ms. An ubuntu-latest GitHub runner parses at about the
//!   same speed, which fuzz CI checks.
//!
//! The limits can't sit much further above the corpus: its slowest file
//! already parses at a quarter of the bound.
//!
//! The fuel lives in a thread-local because `OrgSource`, which every parser
//! advances through, is a plain `Copy` value with nowhere to keep it.

use std::cell::Cell;

use super::OrgSource;
use crate::error::CustomError;

/// Fuel per byte of input.
const PER_BYTE: u64 = 36;
/// Fuel every parse gets on top, so a small input has room.
const BASE: u64 = 20_000;

/// Spent per exit matcher run.
pub(crate) const EXIT_MATCHER: u64 = 2;
/// Spent per attempt to parse an element or list item. An element tries
/// some twenty parsers, which mostly fail without running an exit matcher.
pub(crate) const ELEMENT: u64 = 24;
/// Spent per attempt to parse an object or table cell, for the same reason.
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
        if let Some(left) = self.left.checked_sub(units) {
            self.left = left;
            return;
        }
        self.left = 0;
        self.opener = Some(if self.attempt_depth > 0 {
            self.attempt_start
        } else {
            offset
        });
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

/// Spends `units` on an attempt to parse something at `input`, and fails it
/// once the fuel is spent.
pub(crate) fn spend(units: u64, input: OrgSource<'_>) -> Result<(), nom::Err<CustomError>> {
    if burn(units, input.get_byte_offset()) {
        return Err(out_of_fuel());
    }
    Ok(())
}

fn out_of_fuel() -> nom::Err<CustomError> {
    nom::Err::Failure(CustomError::Static("Out of fuel."))
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
    /// Starts an attempt at `input`, or fails it once the fuel is spent.
    pub(crate) fn start(input: OrgSource<'_>) -> Result<Attempt, nom::Err<CustomError>> {
        let offset = input.get_byte_offset();
        let mut tank = TANK.get();
        if tank.attempt_depth == 0 {
            tank.attempt_start = offset;
        }
        tank.attempt_depth += 1;
        tank.burn(ELEMENT, offset);
        TANK.set(tank);
        // Built first, so its drop ends the attempt when this one fails.
        let attempt = Attempt;
        if tank.opener.is_some() {
            return Err(out_of_fuel());
        }
        Ok(attempt)
    }
}

impl Drop for Attempt {
    fn drop(&mut self) {
        let mut tank = TANK.get();
        tank.attempt_depth -= 1;
        TANK.set(tank);
    }
}
