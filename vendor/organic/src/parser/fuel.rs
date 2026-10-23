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
//! - The corpus needs at most 20.5 units a byte, in ox-hugo's manual (the
//!   next file needs 15.9), and no file needs more than 72 units on top of
//!   36 a byte. The limits below are 1.75 times the first and far above the
//!   second.
//! - Inputs that run out spend up to about 20 ns a unit, among them every
//!   slow input the fuzz targets found, so running out takes at most about
//!   0.7 µs a byte plus 0.2 ms there.
//! - An ubuntu-latest GitHub runner parses 2 to 2.7 times slower, so there
//!   running out can take longer than the bound. Fuzz CI scales the bound
//!   by how much slower the machine parses a reference document; see
//!   `slowdown` in the fuzz crate.
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
const BASE: u64 = 10_000;

/// Spent per exit matcher run.
pub(crate) const EXIT_MATCHER: u64 = 2;
/// Spent per attempt to parse an element or list item. An element tries
/// some twenty parsers, which mostly fail without running an exit matcher.
pub(crate) const ELEMENT: u64 = 24;
/// Spent per attempt to parse an object or table cell, for the same reason.
pub(crate) const OBJECT: u64 = 16;

/// How much stack a parse may use. Org sets no limit on nesting, and the
/// parser recurses for each level, so a deep enough input would overflow
/// the stack, which aborts the process instead of unwinding. The corpus
/// needs at most 102 KB in a release build and 670 KB in a debug one, and
/// 1 MiB leaves room in a 2 MiB thread.
const STACK: usize = 1 << 20;

/// Why a parse stopped early.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stop {
    OutOfFuel,
    TooDeep,
}

#[derive(Clone, Copy)]
struct Tank {
    left: u64,
    /// The stack's address when the parse began.
    stack_base: usize,
    /// Where the outermost element attempt in progress began.
    attempt_start: usize,
    attempt_depth: u32,
    /// Set when the parse stops: why, and where the runaway parse began.
    stopped: Option<(Stop, usize)>,
}

impl Tank {
    fn burn(&mut self, units: u64, offset: usize) {
        if self.stopped.is_some() {
            return;
        }
        let stop = if self.stack_base.saturating_sub(stack_address()) > STACK {
            Stop::TooDeep
        } else if let Some(left) = self.left.checked_sub(units) {
            self.left = left;
            return;
        } else {
            Stop::OutOfFuel
        };
        let opener = if self.attempt_depth > 0 {
            self.attempt_start
        } else {
            offset
        };
        self.stopped = Some((stop, opener));
    }
}

/// About where the stack is now. It grows down on every target yuanme
/// builds for.
#[inline(always)]
fn stack_address() -> usize {
    let marker = 0u8;
    std::hint::black_box(&marker) as *const u8 as usize
}

thread_local! {
    // Unlimited until `fill`, so a parser called on its own in a unit test
    // never stops.
    static TANK: Cell<Tank> = const {
        Cell::new(Tank {
            left: u64::MAX,
            stack_base: 0,
            attempt_start: 0,
            attempt_depth: 0,
            stopped: None,
        })
    };
}

/// Gives this thread's next parse fuel for an input of `len` bytes.
pub(crate) fn fill(len: usize) {
    TANK.set(Tank {
        left: BASE.saturating_add((len as u64).saturating_mul(PER_BYTE)),
        stack_base: stack_address(),
        attempt_start: 0,
        attempt_depth: 0,
        stopped: None,
    });
}

/// Spends `units` of fuel on work at byte `offset`, and says whether the
/// parse has stopped.
pub(crate) fn burn(units: u64, offset: usize) -> bool {
    let mut tank = TANK.get();
    tank.burn(units, offset);
    TANK.set(tank);
    tank.stopped.is_some()
}

/// Spends `units` on an attempt to parse something at `input`, and fails it
/// once the parse has stopped.
pub(crate) fn spend(units: u64, input: OrgSource<'_>) -> Result<(), nom::Err<CustomError>> {
    if burn(units, input.get_byte_offset()) {
        return Err(stopped_error());
    }
    Ok(())
}

fn stopped_error() -> nom::Err<CustomError> {
    nom::Err::Failure(CustomError::Static("The parse stopped."))
}

/// Says whether the parse has stopped, stopping it first if the stack is
/// too deep at byte `offset`.
pub(crate) fn check(offset: usize) -> bool {
    let mut tank = TANK.get();
    if tank.stopped.is_some() {
        return true;
    }
    if tank.stack_base.saturating_sub(stack_address()) <= STACK {
        return false;
    }
    tank.burn(0, offset);
    TANK.set(tank);
    true
}

/// Why this thread's parse stopped early, if it did, and where the runaway
/// parse began.
pub(crate) fn stopped() -> Option<(Stop, usize)> {
    TANK.get().stopped
}

/// Marks an attempt to parse an element at `offset` until it is dropped.
/// The outermost one in progress is the opener a runaway parse reports:
/// the top-level paragraph whose objects ran away, or the first of the
/// unclosed blocks nested in each other.
pub(crate) struct Attempt;

impl Attempt {
    /// Starts an attempt at `input`, or fails it once the parse has stopped.
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
        if tank.stopped.is_some() {
            return Err(stopped_error());
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
