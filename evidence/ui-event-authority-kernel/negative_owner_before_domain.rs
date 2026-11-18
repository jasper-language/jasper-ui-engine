use vstd::prelude::*;
verus! {


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AuthorityFailure {
    IdentityDomain,
    Owner,
    Session,
    Revision,
}

pub(crate) fn authority_failure_from_matches(
    identity_domain_matches: bool,
    owner_matches: bool,
    session_matches: bool,
    revision_matches: bool,
) -> (result: Option<AuthorityFailure>)
    ensures match result {
        Some(AuthorityFailure::IdentityDomain) => !identity_domain_matches,
        Some(AuthorityFailure::Owner) => identity_domain_matches && !owner_matches,
        Some(AuthorityFailure::Session) =>
            identity_domain_matches && owner_matches && !session_matches,
        Some(AuthorityFailure::Revision) =>
            identity_domain_matches && owner_matches && session_matches
            && !revision_matches,
        None => identity_domain_matches && owner_matches
            && session_matches && revision_matches,
    },
{
    if !identity_domain_matches {
        return Some(AuthorityFailure::IdentityDomain);
    }
    if !identity_domain_matches {
        return Some(AuthorityFailure::Owner);
    }
    if !session_matches {
        return Some(AuthorityFailure::Session);
    }
    if !revision_matches {
        return Some(AuthorityFailure::Revision);
    }
    None
}

}
