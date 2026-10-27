//! Verification of the unexported capture-store candidate, not production routing.
#![allow(dead_code)]
include!("types.rs");
include!("../../../experiments/capture-slots/slots.rs");

fn value(target: Option<u64>, path: Vec<u64>) -> Capture {
    // Avoid std's in-place collect specialization in this harness; preserve every
    // supplied path element. The included candidate runtime is unchanged.
    let mut nodes = Vec::new();
    for id in path { nodes.push(NodeId(id)); }
    Capture { target: target.map(NodeId), path: nodes }
}
fn equal(actual: Option<&Capture>, expected: Option<&Capture>) {
    assert_eq!(actual.map(|c| c.target), expected.map(|c| c.target));
    assert_eq!(actual.map(|c| &c.path), expected.map(|c| &c.path));
}

/// Full-width two-key functional target, normal allocation and destruction.
/// Duplicate key overwrites old target/path; removal preserves the other key.
pub fn check_two_keys(first: u64, second: u64, query: u64,
                      first_target: Option<u64>, second_target: Option<u64>) {
    let mut slots = CaptureSlots::new();
    assert!(slots.insert(first, value(first_target, vec![])).is_none());
    let previous = slots.insert(second, value(second_target, vec![]));
    if first == second {
        equal(previous.as_ref(), Some(&value(first_target, vec![])));
    } else { assert!(previous.is_none()); }
    assert_eq!(slots.len(), if first == second { 1 } else { 2 });
    let expected = if query == second { Some(value(second_target, vec![])) }
        else if query == first { Some(value(first_target, vec![])) } else { None };
    equal(slots.get(query), expected.as_ref());
    let removed = slots.remove(query);
    equal(removed.as_ref(), expected.as_ref());
    assert!(slots.get(query).is_none());
    assert_eq!(slots.len(), (if first == second { 1 } else { 2 }) - usize::from(expected.is_some()));
    if first != query {
        equal(slots.get(first), Some(&value(if first == second {second_target} else {first_target}, vec![])));
    }
    if second != query { equal(slots.get(second), Some(&value(second_target, vec![]))); }
}

/// Byte-stream operation driver shared by properties and libFuzzer.
/// Each operation checks the real candidate against std BTreeMap, including paths.
pub fn check_operations(data: &[u8]) {
    use std::collections::BTreeMap;
    let mut slots = CaptureSlots::new();
    let mut oracle: BTreeMap<u64, Capture> = BTreeMap::new();
    let read = |bytes: &[u8]| u64::from_le_bytes(bytes.try_into().unwrap());
    for chunk in data.chunks_exact(34).take(64) {
        let key = read(&chunk[1..9]);
        let target = if chunk[9] & 1 == 0 {None} else {Some(read(&chunk[10..18]))};
        let capture = value(target, vec![read(&chunk[18..26]), read(&chunk[26..34])]);
        match chunk[0] % 3 {
            0 => equal(slots.insert(key, capture.clone()).as_ref(), oracle.insert(key, capture).as_ref()),
            1 => equal(slots.remove(key).as_ref(), oracle.remove(&key).as_ref()),
            _ => equal(slots.get(key), oracle.get(&key)),
        }
        assert_eq!(slots.len(), oracle.len());
        for (&stored_key, stored) in &oracle { equal(slots.get(stored_key), Some(stored)); }
        for i in 0..slots.entries.len() {
            for j in i+1..slots.entries.len() { assert_ne!(slots.entries[i].0, slots.entries[j].0); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn arbitrary_two_keys(a in any::<u64>(), b in any::<u64>(), q in any::<u64>(),
            x in proptest::option::of(any::<u64>()), y in proptest::option::of(any::<u64>())) {
            check_two_keys(a,b,q,x,y);
            check_two_keys(a,b,a,x,y);
            check_two_keys(a,b,b,x,y);
            check_two_keys(a,a,a,x,y);
        }
        #[test]
        fn operation_sequences(data in proptest::collection::vec(any::<u8>(), 0..2177)) {
            check_operations(&data);
        }
    }
    #[test]
    fn miri_boundaries_and_repeated_removal() {
        for (a,b,q) in [(0,0,0),(0,u64::MAX,0),(0,u64::MAX,u64::MAX),(1,2,3)] {
            check_two_keys(a,b,q,None,Some(u64::MAX));
        }
        let mut store=CaptureSlots::new();
        for n in [0,u64::MAX,9] { store.insert(n,value(Some(n),vec![n,0,u64::MAX])); }
        equal(store.insert(9,value(None,vec![])).as_ref(),Some(&value(Some(9),vec![9,0,u64::MAX])));
        for n in [0,u64::MAX,9] { assert!(store.remove(n).is_some()); assert!(store.remove(n).is_none()); }
        assert_eq!(store.len(),0);
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;
    // Partition independent obligations, keeping full-width inputs and real drop.
    #[kani::proof]
    #[kani::unwind(3)]
    fn two_keys_query_normal_drop() {
        let (a,b,q): (u64,u64,u64) = kani::any();
        let (x,y): (Option<u64>,Option<u64>) = kani::any();
        let mut slots=CaptureSlots::new();
        assert!(slots.insert(a,Capture { target: x.map(NodeId), path: Vec::new() }).is_none());
        let previous=slots.insert(b,Capture { target: y.map(NodeId), path: Vec::new() });
        assert_eq!(previous.as_ref().map(|c| c.target), if a==b {Some(x.map(NodeId))} else {None});
        assert_eq!(slots.len(),if a==b {1} else {2});
        let expected=if q==b {Some(y.map(NodeId))} else if q==a {Some(x.map(NodeId))} else {None};
        assert_eq!(slots.get(q).map(|c| c.target),expected);
        assert!(slots.get(q).map_or(true, |c| c.path.is_empty()));
    }
    #[kani::proof]
    #[kani::unwind(3)]
    fn two_keys_remove_normal_drop() {
        let (a,b,q): (u64,u64,u64) = kani::any();
        let (x,y): (Option<u64>,Option<u64>) = kani::any();
        let mut slots=CaptureSlots::new();
        slots.insert(a,Capture { target: x.map(NodeId), path: Vec::new() });
        slots.insert(b,Capture { target: y.map(NodeId), path: Vec::new() });
        let expected=if q==b {Some(y.map(NodeId))} else if q==a {Some(x.map(NodeId))} else {None};
        let removed=slots.remove(q);
        assert_eq!(removed.as_ref().map(|c| c.target),expected);
        assert!(removed.as_ref().map_or(true, |c| c.path.is_empty()));
        assert!(slots.get(q).is_none());
        assert_eq!(slots.len(),(if a==b {1} else {2})-usize::from(expected.is_some()));
        if a!=q {
            assert_eq!(slots.get(a).map(|c| c.target),Some(if a==b {y} else {x}.map(NodeId)));
        }
        if b!=q { assert_eq!(slots.get(b).map(|c| c.target),Some(y.map(NodeId))); }
    }
}
