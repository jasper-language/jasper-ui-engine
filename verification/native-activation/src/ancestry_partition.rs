//! Exploratory parent-class partition; not full-domain acceptance.
//! 父边等价类分区探测，不冒充原完整输入域验收。
#[kani::proof]
#[kani::unwind(4)]
    fn parents_2_1() {
        use super::*;
        let a:u8=2;let b:u8=1;
        let parent=|v:u8|if v%4==0 {None} else {Some((v%4-1) as u64)};
        let pa=parent(a);let pb=parent(b);
        // Real production map, built directly: exclude unrelated collect/sort/oracle containers.
        let mut parents=BTreeMap::new();parents.insert(NodeId(0),pa.map(NodeId));parents.insert(NodeId(1),pb.map(NodeId));
        let start=(kani::any::<u8>()%4) as u64;let target=(kani::any::<u8>()%4) as u64;
        let actual=source_subtree_contains(&parents,NodeId(start),NodeId(target));
        let mut current=Some(start);let mut expected=false;
        for _ in 0..3 {
            if let Some(node)=current {
                if node==target {expected=true;break;}
                current=if node==0 {pa} else if node==1 {pb} else {None};
            }
        }
        assert_eq!(actual,expected);
        // The production function only borrows the map; destruction is outside this scope.
        std::mem::forget(parents);
    }

#[kani::proof]
#[kani::unwind(4)]
    fn case_2_1_0_3() {
        use super::*;
        let a:u8=2;let b:u8=1;
        let parent=|v:u8|if v%4==0 {None} else {Some((v%4-1) as u64)};
        let pa=parent(a);let pb=parent(b);
        // Real production map, built directly: exclude unrelated collect/sort/oracle containers.
        let mut parents=BTreeMap::new();parents.insert(NodeId(0),pa.map(NodeId));parents.insert(NodeId(1),pb.map(NodeId));
        let start=(0u8%4) as u64;let target=(3u8%4) as u64;
        let actual=source_subtree_contains(&parents,NodeId(start),NodeId(target));
        let mut current=Some(start);let mut expected=false;
        for _ in 0..3 {
            if let Some(node)=current {
                if node==target {expected=true;break;}
                current=if node==0 {pa} else if node==1 {pb} else {None};
            }
        }
        assert_eq!(actual,expected);
        // The production function only borrows the map; destruction is outside this scope.
        std::mem::forget(parents);
    }
