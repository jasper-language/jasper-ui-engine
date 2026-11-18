use vstd::prelude::*;
verus! {




#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceKind {
    Image,
    Svg,
    Font,
    Icon,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceAdmission {
    Prepare,
    Commit,
    Release,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UiResourceTransition {
    Prepare,
    NoOp,
    Commit,
    ReleaseClear,
    ReleaseKeepPrepared,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UiResourceTransitionError {
    KindChanged,
    StaleGeneration,
    GenerationNotPrepared,
    ReleasedGeneration,
    CommitMismatch,
    ReleaseMismatch,
}

fn resource_transition(
    highest_present: bool,
    highest_kind: ResourceKind,
    highest_generation: u64,
    active_present: bool,
    active_kind: ResourceKind,
    active_generation: u64,
    prepared_present: bool,
    prepared_kind: ResourceKind,
    prepared_generation: u64,
    next_kind: ResourceKind,
    next_generation: u64,
    admission: ResourceAdmission,
) -> (result: Result<UiResourceTransition, UiResourceTransitionError>)
    ensures match result {
        Ok(UiResourceTransition::Prepare) => admission == ResourceAdmission::Prepare
            && (!highest_present || (matches!(
                    (highest_kind, next_kind),
                    (ResourceKind::Image, ResourceKind::Image)
                        | (ResourceKind::Svg, ResourceKind::Svg)
                        | (ResourceKind::Font, ResourceKind::Font)
                        | (ResourceKind::Icon, ResourceKind::Icon)
                )
                && next_generation >= highest_generation
                && !(next_generation == highest_generation
                    && ((active_present && matches!(
                            (active_kind, next_kind),
                            (ResourceKind::Image, ResourceKind::Image)
                                | (ResourceKind::Svg, ResourceKind::Svg)
                                | (ResourceKind::Font, ResourceKind::Font)
                                | (ResourceKind::Icon, ResourceKind::Icon)
                        )
                            && active_generation == next_generation)
                        || (prepared_present && matches!(
                                (prepared_kind, next_kind),
                                (ResourceKind::Image, ResourceKind::Image)
                                    | (ResourceKind::Svg, ResourceKind::Svg)
                                    | (ResourceKind::Font, ResourceKind::Font)
                                    | (ResourceKind::Icon, ResourceKind::Icon)
                            )
                            && prepared_generation == next_generation)))
                && !(next_generation == highest_generation
                    && !active_present && !prepared_present))),
        Ok(UiResourceTransition::NoOp) => admission == ResourceAdmission::Prepare
            && highest_present && matches!(
                (highest_kind, next_kind),
                (ResourceKind::Image, ResourceKind::Image)
                    | (ResourceKind::Svg, ResourceKind::Svg)
                    | (ResourceKind::Font, ResourceKind::Font)
                    | (ResourceKind::Icon, ResourceKind::Icon)
            )
            && highest_generation == next_generation
            && ((active_present && matches!(
                    (active_kind, next_kind),
                    (ResourceKind::Image, ResourceKind::Image)
                        | (ResourceKind::Svg, ResourceKind::Svg)
                        | (ResourceKind::Font, ResourceKind::Font)
                        | (ResourceKind::Icon, ResourceKind::Icon)
                )
                    && active_generation == next_generation)
                || (prepared_present && matches!(
                        (prepared_kind, next_kind),
                        (ResourceKind::Image, ResourceKind::Image)
                            | (ResourceKind::Svg, ResourceKind::Svg)
                            | (ResourceKind::Font, ResourceKind::Font)
                            | (ResourceKind::Icon, ResourceKind::Icon)
                    )
                    && prepared_generation == next_generation)),
        Ok(UiResourceTransition::Commit) => admission == ResourceAdmission::Commit
            && highest_present && matches!(
                (highest_kind, next_kind),
                (ResourceKind::Image, ResourceKind::Image)
                    | (ResourceKind::Svg, ResourceKind::Svg)
                    | (ResourceKind::Font, ResourceKind::Font)
                    | (ResourceKind::Icon, ResourceKind::Icon)
            )
            && next_generation <= highest_generation
            && prepared_present && matches!(
                (prepared_kind, next_kind),
                (ResourceKind::Image, ResourceKind::Image)
                    | (ResourceKind::Svg, ResourceKind::Svg)
                    | (ResourceKind::Font, ResourceKind::Font)
                    | (ResourceKind::Icon, ResourceKind::Icon)
            )
            && prepared_generation == next_generation,
        Ok(UiResourceTransition::ReleaseClear) => admission == ResourceAdmission::Release
            && active_present && matches!(
                (active_kind, next_kind),
                (ResourceKind::Image, ResourceKind::Image)
                    | (ResourceKind::Svg, ResourceKind::Svg)
                    | (ResourceKind::Font, ResourceKind::Font)
                    | (ResourceKind::Icon, ResourceKind::Icon)
            )
            && active_generation == next_generation && !prepared_present
            && highest_present && matches!(
                (highest_kind, next_kind),
                (ResourceKind::Image, ResourceKind::Image)
                    | (ResourceKind::Svg, ResourceKind::Svg)
                    | (ResourceKind::Font, ResourceKind::Font)
                    | (ResourceKind::Icon, ResourceKind::Icon)
            )
            && next_generation <= highest_generation,
        Ok(UiResourceTransition::ReleaseKeepPrepared) =>
            admission == ResourceAdmission::Release
            && active_present && matches!(
                (active_kind, next_kind),
                (ResourceKind::Image, ResourceKind::Image)
                    | (ResourceKind::Svg, ResourceKind::Svg)
                    | (ResourceKind::Font, ResourceKind::Font)
                    | (ResourceKind::Icon, ResourceKind::Icon)
            )
            && active_generation == next_generation && prepared_present
            && highest_present && matches!(
                (highest_kind, next_kind),
                (ResourceKind::Image, ResourceKind::Image)
                    | (ResourceKind::Svg, ResourceKind::Svg)
                    | (ResourceKind::Font, ResourceKind::Font)
                    | (ResourceKind::Icon, ResourceKind::Icon)
            )
            && next_generation <= highest_generation,
        Err(UiResourceTransitionError::KindChanged) =>
            highest_present && !matches!(
                (highest_kind, next_kind),
                (ResourceKind::Image, ResourceKind::Image)
                    | (ResourceKind::Svg, ResourceKind::Svg)
                    | (ResourceKind::Font, ResourceKind::Font)
                    | (ResourceKind::Icon, ResourceKind::Icon)
            ),
        Err(UiResourceTransitionError::StaleGeneration) =>
            highest_present && matches!(
                (highest_kind, next_kind),
                (ResourceKind::Image, ResourceKind::Image)
                    | (ResourceKind::Svg, ResourceKind::Svg)
                    | (ResourceKind::Font, ResourceKind::Font)
                    | (ResourceKind::Icon, ResourceKind::Icon)
            )
            && next_generation < highest_generation
            && !matches!(admission, ResourceAdmission::Release),
        Err(UiResourceTransitionError::GenerationNotPrepared) =>
            highest_present && matches!(
                (highest_kind, next_kind),
                (ResourceKind::Image, ResourceKind::Image)
                    | (ResourceKind::Svg, ResourceKind::Svg)
                    | (ResourceKind::Font, ResourceKind::Font)
                    | (ResourceKind::Icon, ResourceKind::Icon)
            )
            && next_generation > highest_generation
            && !matches!(admission, ResourceAdmission::Prepare),
        Err(UiResourceTransitionError::ReleasedGeneration) =>
            admission == ResourceAdmission::Prepare
            && highest_present && matches!(
                (highest_kind, next_kind),
                (ResourceKind::Image, ResourceKind::Image)
                    | (ResourceKind::Svg, ResourceKind::Svg)
                    | (ResourceKind::Font, ResourceKind::Font)
                    | (ResourceKind::Icon, ResourceKind::Icon)
            )
            && highest_generation == next_generation
            && !active_present && !prepared_present,
        Err(UiResourceTransitionError::CommitMismatch) =>
            admission == ResourceAdmission::Commit
            && (!highest_present || (matches!(
                    (highest_kind, next_kind),
                    (ResourceKind::Image, ResourceKind::Image)
                        | (ResourceKind::Svg, ResourceKind::Svg)
                        | (ResourceKind::Font, ResourceKind::Font)
                        | (ResourceKind::Icon, ResourceKind::Icon)
                )
                && next_generation <= highest_generation
                && !(prepared_present && matches!(
                        (prepared_kind, next_kind),
                        (ResourceKind::Image, ResourceKind::Image)
                            | (ResourceKind::Svg, ResourceKind::Svg)
                            | (ResourceKind::Font, ResourceKind::Font)
                            | (ResourceKind::Icon, ResourceKind::Icon)
                    )
                    && prepared_generation == next_generation))),
        Err(UiResourceTransitionError::ReleaseMismatch) =>
            admission == ResourceAdmission::Release
            && (!highest_present || (matches!(
                    (highest_kind, next_kind),
                    (ResourceKind::Image, ResourceKind::Image)
                        | (ResourceKind::Svg, ResourceKind::Svg)
                        | (ResourceKind::Font, ResourceKind::Font)
                        | (ResourceKind::Icon, ResourceKind::Icon)
                )
                && next_generation <= highest_generation
                && !(active_present && matches!(
                        (active_kind, next_kind),
                        (ResourceKind::Image, ResourceKind::Image)
                            | (ResourceKind::Svg, ResourceKind::Svg)
                            | (ResourceKind::Font, ResourceKind::Font)
                            | (ResourceKind::Icon, ResourceKind::Icon)
                    )
                    && active_generation == next_generation))),
    },
{
    if !highest_present {
        return match admission {
            ResourceAdmission::Prepare => Ok(UiResourceTransition::Prepare),
            ResourceAdmission::Commit => Err(UiResourceTransitionError::CommitMismatch),
            ResourceAdmission::Release => Err(UiResourceTransitionError::ReleaseMismatch),
        };
    }
    let same_kind = matches!(
        (next_kind, highest_kind),
        (ResourceKind::Image, ResourceKind::Image)
            | (ResourceKind::Svg, ResourceKind::Svg)
            | (ResourceKind::Font, ResourceKind::Font)
            | (ResourceKind::Icon, ResourceKind::Icon)
    );
    if same_kind {
        if next_generation < highest_generation
            && !matches!(admission, ResourceAdmission::Release)
        {
            return Err(UiResourceTransitionError::StaleGeneration);
        }
        if next_generation > highest_generation
            && !matches!(admission, ResourceAdmission::Prepare)
        {
            return Err(UiResourceTransitionError::GenerationNotPrepared);
        }
        match admission {
            ResourceAdmission::Prepare => {
                if next_generation == highest_generation {
                    let active_matches = active_present
                        && matches!(
                            (active_kind, next_kind),
                            (ResourceKind::Image, ResourceKind::Image)
                                | (ResourceKind::Svg, ResourceKind::Svg)
                                | (ResourceKind::Font, ResourceKind::Font)
                                | (ResourceKind::Icon, ResourceKind::Icon)
                        )
                        && active_generation == next_generation;
                    let prepared_matches = prepared_present
                        && matches!(
                            (prepared_kind, next_kind),
                            (ResourceKind::Image, ResourceKind::Image)
                                | (ResourceKind::Svg, ResourceKind::Svg)
                                | (ResourceKind::Font, ResourceKind::Font)
                                | (ResourceKind::Icon, ResourceKind::Icon)
                        )
                        && prepared_generation == next_generation;
                    if prepared_matches || active_matches {
                        return Ok(UiResourceTransition::NoOp);
                    }
                    if !active_present && !prepared_present {
                        return Ok(UiResourceTransition::Prepare);
                    }
                }
                Ok(UiResourceTransition::Prepare)
            }
            ResourceAdmission::Commit => {
                if prepared_present
                    && matches!(
                        (prepared_kind, next_kind),
                        (ResourceKind::Image, ResourceKind::Image)
                            | (ResourceKind::Svg, ResourceKind::Svg)
                            | (ResourceKind::Font, ResourceKind::Font)
                            | (ResourceKind::Icon, ResourceKind::Icon)
                    )
                    && prepared_generation == next_generation
                {
                    Ok(UiResourceTransition::Commit)
                } else {
                    Err(UiResourceTransitionError::CommitMismatch)
                }
            }
            ResourceAdmission::Release => {
                if !active_present
                    || !matches!(
                        (active_kind, next_kind),
                        (ResourceKind::Image, ResourceKind::Image)
                            | (ResourceKind::Svg, ResourceKind::Svg)
                            | (ResourceKind::Font, ResourceKind::Font)
                            | (ResourceKind::Icon, ResourceKind::Icon)
                    )
                    || active_generation != next_generation
                {
                    return Err(UiResourceTransitionError::ReleaseMismatch);
                }
                if prepared_present {
                    Ok(UiResourceTransition::ReleaseKeepPrepared)
                } else {
                    Ok(UiResourceTransition::ReleaseClear)
                }
            }
        }
    } else {
        Err(UiResourceTransitionError::KindChanged)
    }
}

}
