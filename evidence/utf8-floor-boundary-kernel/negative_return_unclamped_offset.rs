use vstd::prelude::*;
verus! {
use vstd::string::StringSliceAdditionalSpecFns;
broadcast use vstd::utf8::encode_utf8_valid_utf8;

pub(crate) fn floor_utf8_boundary(text: &str, offset: usize) -> (result: usize)
    ensures result <= offset,
        result <= text.spec_bytes().len(),
        vstd::utf8::is_char_boundary(text.spec_bytes(), result as int),
        forall|i: int| result < i <= (if offset <= text.spec_bytes().len() {
                offset as int
            } else {
                text.spec_bytes().len() as int
            }) ==> !vstd::utf8::is_char_boundary(text.spec_bytes(), i),
{
    let text_len = text.as_bytes().len();
    let limit = if offset <= text_len {
        offset
    } else {
        text_len
    };
    let mut current = limit;
    proof {
        assert(text_len as int == text.spec_bytes().len());
        assert(limit <= text.spec_bytes().len());
    }
    while current > 0 && !text.is_char_boundary(current)
        invariant current <= limit,
            limit <= text.spec_bytes().len(),
            forall|i: int| current < i <= limit as int
                ==> !vstd::utf8::is_char_boundary(text.spec_bytes(), i),
        decreases current,
    {
        proof {
            assert(vstd::utf8::is_char_boundary(text.spec_bytes(), 0));
        }
        current -= 1;
    }
    proof {
        assert(limit as int == (if offset <= text.spec_bytes().len() {
            offset as int
        } else {
            text.spec_bytes().len() as int
        }));
    }
    offset
}

}
