use nom::error::ErrorKind;
use nom::error::ParseError;
use nom::IResult;

pub(crate) type Res<T, U> = IResult<T, U, CustomError>;

#[derive(Debug)]
pub enum CustomError {
    Static(#[allow(dead_code)] &'static str),
    IO(#[allow(dead_code)] std::io::Error),
    Parser(#[allow(dead_code)] ErrorKind),
}

impl<I: std::fmt::Debug> ParseError<I> for CustomError {
    fn from_error_kind(_input: I, kind: ErrorKind) -> Self {
        CustomError::Parser(kind)
    }

    fn append(_input: I, _kind: ErrorKind, /*mut*/ other: Self) -> Self {
        // Doesn't do append like VerboseError
        other
    }
}

impl From<std::io::Error> for CustomError {
    fn from(value: std::io::Error) -> Self {
        CustomError::IO(value)
    }
}

impl From<&'static str> for CustomError {
    fn from(value: &'static str) -> Self {
        CustomError::Static(value)
    }
}
