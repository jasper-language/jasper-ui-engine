use vstd::prelude::*;
use vstd::std_specs::cmp::*;
use vstd::laws_cmp::*;
use core::cmp::Ordering;
verus! {
#[derive(Clone,Copy)]
pub struct NodeId(pub u64);
impl PartialEqSpecImpl for NodeId {
    open spec fn obeys_eq_spec()->bool {true}
    open spec fn eq_spec(&self,other:&Self)->bool {self.0==other.0}
}
impl PartialOrdSpecImpl for NodeId {
    open spec fn obeys_partial_cmp_spec()->bool {true}
    open spec fn partial_cmp_spec(&self,other:&Self)->Option<Ordering> {Some(OrdSpec::cmp_spec(&self.0,&other.0))}
}
impl OrdSpecImpl for NodeId {
    open spec fn obeys_cmp_spec()->bool {true}
    open spec fn cmp_spec(&self,other:&Self)->Ordering {OrdSpec::cmp_spec(&self.0,&other.0)}
}
impl ::core::cmp::PartialEq for NodeId {
    #[inline]
    fn eq(&self, other: &Self) -> bool { self.0 == other.0 }
}
impl Eq for NodeId {}
impl ::core::cmp::PartialOrd for NodeId {
    #[inline]
    fn partial_cmp(&self, other: &Self)
        -> ::core::option::Option<::core::cmp::Ordering> {
        ::core::option::Option::Some(::core::cmp::Ord::cmp(self, other))
    }
}
impl ::core::cmp::Ord for NodeId {
    #[inline]
    fn cmp(&self, other: &Self) -> ::core::cmp::Ordering {
        ::core::cmp::Ord::cmp(&self.0, &other.0)
    }
}
proof fn node_id_order_model()
    ensures obeys_cmp::<NodeId>(),
{
    reveal(vstd::laws_eq::obeys_eq_spec_properties);
    reveal(obeys_partial_cmp_spec_properties);
    reveal(obeys_cmp_partial_ord);
    reveal(obeys_cmp_ord);
}
}
