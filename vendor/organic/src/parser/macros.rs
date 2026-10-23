/// Parse an element that has affiliated keywords.
macro_rules! ak_element {
    ($parser:expr, $affiliated_keywords:expr, $post_affiliated_keywords_input: expr, $context: expr, $input: expr, $wrapper: expr) => {
        if let Ok((remaining, ele)) = $parser(
            $affiliated_keywords,
            $post_affiliated_keywords_input,
            $context,
            $input,
        ) {
            return Ok((remaining, $wrapper(ele)));
        }
    };
    ($parser:expr, $affiliated_keywords:expr, $post_affiliated_keywords_input: expr, $context: expr, $input: expr) => {
        if let Ok((remaining, ele)) = $parser(
            $affiliated_keywords,
            $post_affiliated_keywords_input,
            $context,
            $input,
        ) {
            return Ok((remaining, ele));
        }
    };
}

pub(crate) use ak_element;

macro_rules! element {
    ($parser:expr, $context: expr, $input: expr, $wrapper: expr) => {
        if let Ok((remaining, ele)) = $parser($context, $input) {
            return Ok((remaining, $wrapper(ele)));
        }
    };
    ($parser:expr, $context: expr, $input: expr) => {
        if let Ok((remaining, ele)) = $parser($context, $input) {
            return Ok((remaining, ele));
        }
    };
    ($parser:expr, $input: expr) => {
        if let Ok((remaining, ele)) = $parser($input) {
            return Ok((remaining, ele));
        }
    };
}

pub(crate) use element;
