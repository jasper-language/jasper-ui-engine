use vstd::prelude::*;
verus! {

pub(crate) fn preedit_buffer_offset_base(
    caret: usize,
    display: usize,
    preedit_len: usize,
) -> (result: usize)
    ensures result <= display,
        display <= caret ==> result == display,
        display > caret && display - caret <= preedit_len ==> result == caret,
        display > caret && display - caret > preedit_len
            ==> result as int == display as int - preedit_len as int,
{
    if display <= caret {
        display
    } else if display - caret <= preedit_len {
        caret
    } else {
        caret
    }
}

}
