"""Generate annotations over the candidate body; no executable substitute.
在候选运行体上生成证明注解，不引入替代运行实现。
"""
from pathlib import Path
import hashlib,json
root = Path(__file__).resolve().parents[2]
source = (root / 'experiments/capture-slots/slots.rs').read_text()
node = (root / 'src/scene.rs').read_text().split('// BEGIN VERIFIED NODE ID TYPE')[1].split('// END VERIFIED NODE ID TYPE')[0]
capture = (root / 'src/pointer.rs').read_text().split('// BEGIN VERIFIED POINTER ROUTER TYPE')[1].split('/// Routes pointer events')[0]
s = source.replace('fn new() -> Self {', 'fn new() -> (result: Self)\n ensures result.entries@.len() == 0,\n{')
s = s.replace('fn len(&self) -> usize {', 'fn len(&self) -> (result: usize)\n ensures result as int == self.entries@.len(),\n{')
s = s.replace('fn get(&self, key: u64) -> Option<&Capture> {', '''fn get(&self, key: u64) -> (result: Option<&Capture>)
 ensures match result {
  Some(value) => exists|i: int| 0 <= i < self.entries@.len()
   && self.entries@[i].0 == key && *value == self.entries@[i].1
   && forall|j: int| 0 <= j < i ==> self.entries@[j].0 != key,
  None => forall|j: int| 0 <= j < self.entries@.len() ==> self.entries@[j].0 != key,
 },
{''')
s = s.replace('fn insert(&mut self, key: u64, value: Capture) -> Option<Capture> {', '''fn insert(&mut self, key: u64, value: Capture) -> (result: Option<Capture>)
 ensures match result {
  Some(previous) => exists|i: int| 0 <= i < old(self).entries@.len()
   && old(self).entries@[i].0 == key && previous == old(self).entries@[i].1
   && (forall|j: int| 0 <= j < i ==> old(self).entries@[j].0 != key)
   && final(self).entries@ == old(self).entries@.update(i, (key, value)),
  None => (forall|j: int| 0 <= j < old(self).entries@.len() ==> old(self).entries@[j].0 != key)
   && final(self).entries@ == old(self).entries@.push((key, value)),
 },
{''')
s = s.replace('fn remove(&mut self, key: u64) -> Option<Capture> {', '''fn remove(&mut self, key: u64) -> (result: Option<Capture>)
 ensures match result {
  Some(previous) => exists|i: int| 0 <= i < old(self).entries@.len()
   && old(self).entries@[i].0 == key && previous == old(self).entries@[i].1
   && (forall|j: int| 0 <= j < i ==> old(self).entries@[j].0 != key)
   && final(self).entries@ == old(self).entries@.remove(i),
  None => (forall|j: int| 0 <= j < old(self).entries@.len() ==> old(self).entries@[j].0 != key)
   && final(self).entries@ == old(self).entries@,
 },
{''')
s = s.replace('fn position(&self, key: u64) -> Option<usize> {', '''fn position(&self, key: u64) -> (result: Option<usize>)
 ensures match result {
  Some(i) => i < self.entries@.len() && self.entries@[i as int].0 == key
   && forall|j: int| 0 <= j < i ==> self.entries@[j].0 != key,
  None => forall|j: int| 0 <= j < self.entries@.len() ==> self.entries@[j].0 != key,
 },
{''')
s = s.replace('while index < self.entries.len() {', '''while index < self.entries.len()
 invariant index <= self.entries@.len(),
  forall|j: int| 0 <= j < index ==> self.entries@[j].0 != key,
 decreases self.entries@.len() - index,
{''')
s = s.replace('match self.position(key) {\n            Some(index) => {', 'let ghost before = self.entries@;\n        match self.position(key) {\n            Some(index) => {')
s = s.replace('self.entries.insert(index, (key, value));', '''self.entries.insert(index, (key, value));
                proof {
                    assert(self.entries@ =~= before.update(index as int, (key, value)));
                }''')
# Reachable storage states have unique keys; this is a ghost-only closure claim.
s = s.replace('ensures result.entries@.len() == 0,', 'ensures result.entries@.len() == 0, unique(result.entries@),')
s = s.replace('ensures match result {\n  Some(previous)', 'ensures unique(old(self).entries@) ==> unique(final(self).entries@),\n match result {\n  Some(previous)')
lemmas = r"""
spec fn unique(s: Seq<(u64, Capture)>) -> bool {
    forall|a: int, b: int| 0 <= a < b < s.len() ==> s[a].0 != s[b].0
}
proof fn unique_update(s: Seq<(u64, Capture)>, i: int, v: (u64, Capture))
    requires unique(s), 0 <= i < s.len(), s[i].0 == v.0,
    ensures unique(s.update(i, v)),
{
    assert forall|a: int, b: int| 0 <= a < b < s.update(i, v).len()
        implies s.update(i, v)[a].0 != s.update(i, v)[b].0 by {
        assert(s[a].0 != s[b].0);
    }
}
proof fn unique_push(s: Seq<(u64, Capture)>, v: (u64, Capture))
    requires unique(s), forall|i: int| 0 <= i < s.len() ==> s[i].0 != v.0,
    ensures unique(s.push(v)),
{
    assert forall|a: int, b: int| 0 <= a < b < s.push(v).len()
        implies s.push(v)[a].0 != s.push(v)[b].0 by {
        if b < s.len() { assert(s[a].0 != s[b].0); }
        else { assert(b == s.len()); assert(s[a].0 != v.0); }
    }
}
proof fn unique_remove(s: Seq<(u64, Capture)>, i: int)
    requires unique(s), 0 <= i < s.len(),
    ensures unique(s.remove(i)),
{
    assert forall|a: int, b: int| 0 <= a < b < s.remove(i).len()
        implies s.remove(i)[a].0 != s.remove(i)[b].0 by {
        let x = if a < i { a } else { a + 1 };
        let y = if b < i { b } else { b + 1 };
        assert(0 <= x < y < s.len());
        assert(s[x].0 != s[y].0);
        assert(s.remove(i)[a] == s[x]);
        assert(s.remove(i)[b] == s[y]);
    }
}
"""
s = lemmas + s
s = s.replace('assert(self.entries@ =~= before.update(index as int, (key, value)));', 'assert(self.entries@ =~= before.update(index as int, (key, value)));\n                    if unique(before) { unique_update(before, index as int, (key, value)); }')
s = s.replace('self.entries.push((key, value));', 'self.entries.push((key, value));\n                proof { if unique(before) { unique_push(before, (key, value)); } }')
s = s.replace('Some(index) => Some(self.entries.remove(index).1),', 'Some(index) => {\n                let ghost before = self.entries@;\n                let previous = self.entries.remove(index);\n                proof { if unique(before) { unique_remove(before, index as int); } }\n                Some(previous.1)\n            },')
out = root / 'evidence/capture-slots-candidate'
out.mkdir(parents=True, exist_ok=True)
(out/'positive.rs').write_text('use vstd::prelude::*;\nverus! {\n'+node+capture+s+'\n}\nfn main() {}\n')
(out/'wrong-index.rs').write_text((out/'positive.rs').read_text().replace('return Some(index);', 'return Some(self.entries.len());'))
(out/'wrong-replacement.rs').write_text((out/'positive.rs').read_text().replace('Some(previous.1)', 'None', 1))
(out/'wrong-removal.rs').write_text((out/'positive.rs').read_text().replace('let ghost before = self.entries@;\n                let previous = self.entries.remove(index);', 'let ghost before = self.entries@;\n                let previous = self.entries.remove(0);', 1))
(out/'scope.json').write_text(json.dumps({'scope':'candidate constructor, len, borrowed get, insert, remove and first-match position over actual Capture/NodeId types extracted from production; unique-key invariant closed under constructor/insert/remove; not complete map equivalence or production routing','candidate_sha256':hashlib.sha256(source.encode()).hexdigest(),'production_integration_complete':False,'six_stage_complete':False},indent=2)+'\n')
