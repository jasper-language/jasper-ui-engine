use vstd::prelude::*;
verus! {



#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UiKey {
    pub semantic_id: u64,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityError {
    InvalidDomain,
    AlreadyLive(u64),
    GenerationExhausted(u64),
    Unknown(UiKey),
    Stale(UiKey),
    Retired(UiKey),
    InvalidSnapshot,
    UnexpectedGeneration { expected: UiKey, actual: UiKey },
}

pub(crate) fn next_ui_identity(
    semantic_id: u64,
    last_generation: u64,
    live: bool,
) -> (result: Result<UiKey, IdentityError>)
    ensures match result {
        Ok(key) => !live && last_generation < u64::MAX
            && key.semantic_id == semantic_id
            && key.generation as int == last_generation as int + 1,
        Err(IdentityError::AlreadyLive(id)) => live && id == semantic_id,
        Err(IdentityError::GenerationExhausted(id)) => !live
            && last_generation == u64::MAX && id == semantic_id,
        Err(_) => false,
    },
{
    if live {
        return Err(IdentityError::AlreadyLive(semantic_id));
    }
    let Some(generation) = last_generation.checked_add(1) else {
        return Err(IdentityError::GenerationExhausted(semantic_id));
    };
    Ok(UiKey {
        semantic_id,
        generation,
    })
}

}
